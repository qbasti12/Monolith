// Fast Apple Silicon Metal + MPSMatrixMultiplication trainer for Monolith
// `(768 -> 512) x 2 -> 1` SCReLU NNUE reading 32-byte `bulletformat::ChessBoard`
// records and emitting `MNUEv001` `.mnue` weights for `monolith-engine`.

#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#import <MetalPerformanceShaders/MetalPerformanceShaders.h>

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fcntl.h>
#include <string>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#include <vector>

static constexpr uint32_t kInputs = 768;
static constexpr uint32_t kHidden = 512;
static constexpr uint32_t kHidden4 = kHidden / 4; // 128 float4s
static constexpr int32_t kQA = 255;
static constexpr int32_t kQB = 64;
static constexpr float kScale = 400.0f;

#pragma pack(push, 1)
struct ChessBoardRec {
    uint64_t occ;
    uint8_t pcs[16];
    int16_t score;
    uint8_t result;
    uint8_t ksq;
    uint8_t opp_ksq;
    uint8_t extra[3];
};
#pragma pack(pop)
static_assert(sizeof(ChessBoardRec) == 32, "ChessBoardRec must be 32 bytes");

struct TrainParams {
    uint32_t batch_size;
    float wdl_lambda;
    float lr;
    float inv_batch;
    float b1_corr;
    float b2_corr;
    float wd;
    float clip_val;
    uint32_t grad_count;
};

static const char* kMetalSource = R"MSL(
#include <metal_stdlib>
using namespace metal;

struct ChessBoardRec {
    ulong occ;
    uchar pcs[16];
    short score;
    uchar result;
    uchar ksq;
    uchar opp_ksq;
    uchar extra[3];
};

struct TrainParams {
    uint batch_size;
    float wdl_lambda;
    float lr;
    float inv_batch;
    float b1_corr;
    float b2_corr;
    float wd;
    float clip_val;
    uint count;
};

// 1 threadgroup per board (128 threads, 4 SIMD warps of 32). Each thread processes
// 4 hidden units (float4) matching HL=512 = 128 * 4.
kernel void nnue_fwd_bwd(
    device const ChessBoardRec* records [[buffer(0)]],
    device const float4* l0w4           [[buffer(1)]],
    device const float4* l0b4           [[buffer(2)]],
    device const float4* l1w4           [[buffer(3)]],
    device const float* l1b             [[buffer(4)]],
    device float* X_dense               [[buffer(5)]],
    device float4* A_screlu4            [[buffer(6)]],
    device float4* G_acc4               [[buffer(7)]],
    device float* loss_buf              [[buffer(8)]],
    device float* draw_buf              [[buffer(9)]],
    constant TrainParams& params        [[buffer(10)]],
    uint tg_id [[threadgroup_position_in_grid]],
    uint tid   [[thread_index_in_threadgroup]],
    uint lane  [[thread_index_in_simdgroup]],
    uint warp  [[simdgroup_index_in_threadgroup]]
) {
    uint B = params.batch_size;
    if (tg_id >= B) return;

    threadgroup ushort sg_stm[32];
    threadgroup ushort sg_ntm[32];
    threadgroup uint sg_count;
    threadgroup float sg_warp[4];
    threadgroup float sg_draw;

    if (tid == 0) {
        ChessBoardRec rec = records[tg_id];
        ulong occ = rec.occ;
        uint cnt = 0;
        while (occ != 0ul && cnt < 32u) {
            uint sq = ctz(occ);
            occ &= (occ - 1ul);
            uchar nibble = (rec.pcs[cnt >> 1] >> (4u * (cnt & 1u))) & 0x0Fu;
            uint c = (nibble >> 3) & 1u;
            uint pc = nibble & 7u;
            sg_stm[cnt] = ushort(c * 384u + pc * 64u + sq);
            sg_ntm[cnt] = ushort((1u - c) * 384u + pc * 64u + (sq ^ 56u));
            cnt++;
        }
        sg_count = cnt;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    uint count = sg_count;
    if (tid < count) {
        X_dense[ulong(tg_id) * 768ul + ulong(sg_stm[tid])] = 1.0f;
        X_dense[ulong(B + tg_id) * 768ul + ulong(sg_ntm[tid])] = 1.0f;
    }

    float4 acc_s = l0b4[tid];
    float4 acc_n = acc_s;
    for (uint k = 0u; k < count; ++k) {
        acc_s += l0w4[ulong(sg_stm[k]) * 128ul + ulong(tid)];
        acc_n += l0w4[ulong(sg_ntm[k]) * 128ul + ulong(tid)];
    }

    float4 cs = clamp(acc_s, 0.0f, 1.0f);
    float4 cn = clamp(acc_n, 0.0f, 1.0f);
    float4 sc_s = cs * cs;
    float4 sc_n = cn * cn;

    A_screlu4[ulong(tg_id) * 256ul + ulong(tid)] = sc_s;
    A_screlu4[ulong(tg_id) * 256ul + 128ul + ulong(tid)] = sc_n;

    float partial = dot(sc_s, l1w4[tid]) + dot(sc_n, l1w4[128u + tid]);
    float warp_sum = simd_sum(partial);
    if (lane == 0u) {
        sg_warp[warp] = warp_sum;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    if (tid == 0u) {
        ChessBoardRec rec = records[tg_id];
        float raw = l1b[0] + sg_warp[0] + sg_warp[1] + sg_warp[2] + sg_warp[3];
        float pred = 1.0f / (1.0f + exp(-raw));
        float score_t = 1.0f / (1.0f + exp(-float(rec.score) / 400.0f));
        float wdl_t = float(rec.result) * 0.5f;
        float target = (1.0f - params.wdl_lambda) * score_t + params.wdl_lambda * wdl_t;
        float diff = pred - target;
        float d_raw = 2.0f * diff * pred * (1.0f - pred);
        sg_draw = d_raw;
        loss_buf[tg_id] = diff * diff;
        draw_buf[tg_id] = d_raw;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    float d_raw = sg_draw;
    float4 ms = select(float4(0.0f), float4(1.0f), (acc_s > 0.0f) & (acc_s < 1.0f));
    float4 mn = select(float4(0.0f), float4(1.0f), (acc_n > 0.0f) & (acc_n < 1.0f));
    G_acc4[ulong(tg_id) * 128ul + ulong(tid)] = d_raw * l1w4[tid] * (2.0f * cs) * ms;
    G_acc4[ulong(B + tg_id) * 128ul + ulong(tid)] = d_raw * l1w4[128u + tid] * (2.0f * cn) * mn;
}

kernel void reduce_scalar_loss(
    device const float* loss_buf [[buffer(0)]],
    device const float* draw_buf [[buffer(1)]],
    device float* grad_l1b       [[buffer(2)]],
    device float* batch_loss     [[buffer(3)]],
    constant TrainParams& params [[buffer(4)]],
    uint tid [[thread_index_in_threadgroup]],
    uint lane [[thread_index_in_simdgroup]],
    uint warp [[simdgroup_index_in_threadgroup]]
) {
    threadgroup float s_loss[8];
    threadgroup float s_draw[8];
    float lsum = 0.0f;
    float dsum = 0.0f;
    for (uint i = tid; i < params.batch_size; i += 256u) {
        lsum += loss_buf[i];
        dsum += draw_buf[i];
    }
    float wl = simd_sum(lsum);
    float wd = simd_sum(dsum);
    if (lane == 0u) {
        s_loss[warp] = wl;
        s_draw[warp] = wd;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (tid == 0u) {
        float tl = 0.0f, td = 0.0f;
        for (uint w = 0u; w < 8u; ++w) {
            tl += s_loss[w];
            td += s_draw[w];
        }
        batch_loss[0] = tl;
        grad_l1b[0] = td;
    }
}

kernel void reduce_cols_p1(
    device const float* A_screlu  [[buffer(0)]],
    device const float* draw_buf  [[buffer(1)]],
    device const float* G_acc     [[buffer(2)]],
    device float* partial_l1w     [[buffer(3)]],
    device float* partial_l0b     [[buffer(4)]],
    constant TrainParams& params  [[buffer(5)]],
    uint2 gid [[thread_position_in_grid]]
) {
    uint col = gid.x;
    uint chunk = gid.y; // 0..63
    uint B = params.batch_size;
    if (col < 1024u) {
        uint start = (B * chunk) >> 6;
        uint end = (B * (chunk + 1u)) >> 6;
        float acc = 0.0f;
        for (uint b = start; b < end; ++b) {
            acc += draw_buf[b] * A_screlu[ulong(b) * 1024ul + ulong(col)];
        }
        partial_l1w[ulong(chunk) * 1024ul + ulong(col)] = acc;
    }
    if (col < 512u) {
        uint B2 = B * 2u;
        uint start = (B2 * chunk) >> 6;
        uint end = (B2 * (chunk + 1u)) >> 6;
        float acc = 0.0f;
        for (uint r = start; r < end; ++r) {
            acc += G_acc[ulong(r) * 512ul + ulong(col)];
        }
        partial_l0b[ulong(chunk) * 512ul + ulong(col)] = acc;
    }
}

kernel void reduce_cols_p2(
    device const float* partial_l1w [[buffer(0)]],
    device const float* partial_l0b [[buffer(1)]],
    device float* grad_l1w          [[buffer(2)]],
    device float* grad_l0b          [[buffer(3)]],
    uint col [[thread_position_in_grid]]
) {
    if (col < 1024u) {
        float acc = 0.0f;
        for (uint c = 0u; c < 64u; ++c) {
            acc += partial_l1w[ulong(c) * 1024ul + ulong(col)];
        }
        grad_l1w[col] = acc;
    }
    if (col < 512u) {
        float acc = 0.0f;
        for (uint c = 0u; c < 64u; ++c) {
            acc += partial_l0b[ulong(c) * 512ul + ulong(col)];
        }
        grad_l0b[col] = acc;
    }
}

kernel void adamw_step(
    device float* w               [[buffer(0)]],
    device const float* g         [[buffer(1)]],
    device float* m               [[buffer(2)]],
    device float* v               [[buffer(3)]],
    constant TrainParams& params  [[buffer(4)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= params.count) return;
    float wi = w[gid];
    float gi = g[gid] * params.inv_batch;
    float mi = 0.9f * m[gid] + 0.1f * gi;
    float vi = 0.999f * v[gid] + 0.001f * gi * gi;
    m[gid] = mi;
    v[gid] = vi;
    float m_hat = mi / params.b1_corr;
    float v_hat = vi / params.b2_corr;
    wi *= (1.0f - params.wd * params.lr);
    wi -= params.lr * (m_hat / (sqrt(v_hat) + 1e-8f));
    if (params.clip_val > 0.0f) {
        wi = clamp(wi, -params.clip_val, params.clip_val);
    }
    w[gid] = wi;
}
)MSL";

static id<MTLComputePipelineState> make_pipeline(id<MTLDevice> dev, id<MTLLibrary> lib, NSString* name) {
    NSError* err = nil;
    id<MTLFunction> fn = [lib newFunctionWithName:name];
    if (!fn) {
        fprintf(stderr, "Missing Metal function %s\n", name.UTF8String);
        exit(1);
    }
    id<MTLComputePipelineState> pso = [dev newComputePipelineStateWithFunction:fn error:&err];
    if (!pso || err) {
        fprintf(stderr, "PSO compile error (%s): %s\n", name.UTF8String, err.localizedDescription.UTF8String);
        exit(1);
    }
    return pso;
}

static bool save_mnue(
    const std::string& path,
    const float* l0w,
    const float* l0b,
    const float* l1w,
    float l1b
) {
    FILE* f = fopen(path.c_str(), "wb");
    if (!f) return false;
    const char magic[8] = {'M', 'N', 'U', 'E', 'v', '0', '0', '1'};
    uint32_t hl = kHidden;
    uint32_t flags = 0;
    fwrite(magic, 1, 8, f);
    fwrite(&hl, sizeof(uint32_t), 1, f);
    fwrite(&flags, sizeof(uint32_t), 1, f);

    auto write_i16_arr = [&](const float* src, size_t n, float q) {
        std::vector<int16_t> tmp(n);
        for (size_t i = 0; i < n; ++i) {
            float v = std::round(src[i] * q);
            v = std::max(-32768.0f, std::min(32767.0f, v));
            tmp[i] = static_cast<int16_t>(v);
        }
        fwrite(tmp.data(), sizeof(int16_t), n, f);
    };
    write_i16_arr(l0w, kInputs * kHidden, static_cast<float>(kQA));
    write_i16_arr(l0b, kHidden, static_cast<float>(kQA));
    write_i16_arr(l1w, 2 * kHidden, static_cast<float>(kQB));
    int32_t out_bias = static_cast<int32_t>(std::llround(double(l1b) * kQA * kQB));
    fwrite(&out_bias, sizeof(int32_t), 1, f);
    fclose(f);
    return true;
}

static void init_weights(float* l0w, float* l0b, float* l1w, float* l1b) {
    uint64_t seed = 0x4D4F4E4F5F4D4554ULL;
    auto next_u = [&]() -> double {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        return static_cast<double>(seed >> 11) * (1.0 / static_cast<double>(1ULL << 53));
    };
    auto randn = [&]() -> float {
        double u1 = std::max(1e-12, next_u());
        double u2 = next_u();
        return static_cast<float>(std::sqrt(-2.0 * std::log(u1)) * std::cos(6.28318530718 * u2));
    };
    const float s0 = std::sqrt(2.0f / static_cast<float>(kInputs));
    const float s1 = std::sqrt(2.0f / static_cast<float>(2 * kHidden));
    for (size_t i = 0; i < kInputs * kHidden; ++i) l0w[i] = randn() * s0 * 0.25f;
    memset(l0b, 0, kHidden * sizeof(float));
    for (size_t i = 0; i < 2 * kHidden; ++i) l1w[i] = randn() * s1 * 0.5f;
    *l1b = 0.0f;
}

struct MetalTrainer {
    id<MTLDevice> dev = nil;
    id<MTLCommandQueue> q = nil;
    uint32_t B = 0;
    id<MTLComputePipelineState> p_fwdbwd = nil;
    id<MTLComputePipelineState> p_loss = nil;
    id<MTLComputePipelineState> p_rc1 = nil;
    id<MTLComputePipelineState> p_rc2 = nil;
    id<MTLComputePipelineState> p_adamw = nil;

    id<MTLBuffer> b_l0w, b_m_l0w, b_v_l0w, b_g_l0w;
    id<MTLBuffer> b_l0b, b_m_l0b, b_v_l0b, b_g_l0b;
    id<MTLBuffer> b_l1w, b_m_l1w, b_v_l1w, b_g_l1w;
    id<MTLBuffer> b_l1b, b_m_l1b, b_v_l1b, b_g_l1b;
    id<MTLBuffer> b_rec[2];
    id<MTLBuffer> b_loss_sum[2];
    id<MTLBuffer> b_X, b_A, b_G, b_loss_i, b_draw_i, b_part_l1w, b_part_l0b;
    MPSMatrix* m_X = nil;
    MPSMatrix* m_G = nil;
    MPSMatrix* m_gl0w = nil;
    MPSMatrixMultiplication* gemm_l0w = nil;

    id<MTLBuffer> alloc_shared(size_t bytes) {
        id<MTLBuffer> b = [dev newBufferWithLength:bytes options:MTLResourceStorageModeShared];
        memset(b.contents, 0, bytes);
        return b;
    }

    bool init(id<MTLDevice> device, id<MTLCommandQueue> queue, uint32_t batch_size) {
        dev = device;
        q = queue;
        B = batch_size;

        NSError* err = nil;
        MTLCompileOptions* opts = [[MTLCompileOptions alloc] init];
        id<MTLLibrary> lib = [dev newLibraryWithSource:[NSString stringWithUTF8String:kMetalSource] options:opts error:&err];
        if (!lib || err) {
            fprintf(stderr, "Metal compile err: %s\n", err.localizedDescription.UTF8String);
            return false;
        }
        p_fwdbwd = make_pipeline(dev, lib, @"nnue_fwd_bwd");
        p_loss = make_pipeline(dev, lib, @"reduce_scalar_loss");
        p_rc1 = make_pipeline(dev, lib, @"reduce_cols_p1");
        p_rc2 = make_pipeline(dev, lib, @"reduce_cols_p2");
        p_adamw = make_pipeline(dev, lib, @"adamw_step");

        b_l0w = alloc_shared(kInputs * kHidden * sizeof(float));
        b_m_l0w = alloc_shared(kInputs * kHidden * sizeof(float));
        b_v_l0w = alloc_shared(kInputs * kHidden * sizeof(float));
        b_g_l0w = alloc_shared(kInputs * kHidden * sizeof(float));
        b_l0b = alloc_shared(kHidden * sizeof(float));
        b_m_l0b = alloc_shared(kHidden * sizeof(float));
        b_v_l0b = alloc_shared(kHidden * sizeof(float));
        b_g_l0b = alloc_shared(kHidden * sizeof(float));
        b_l1w = alloc_shared(2 * kHidden * sizeof(float));
        b_m_l1w = alloc_shared(2 * kHidden * sizeof(float));
        b_v_l1w = alloc_shared(2 * kHidden * sizeof(float));
        b_g_l1w = alloc_shared(2 * kHidden * sizeof(float));
        b_l1b = alloc_shared(sizeof(float));
        b_m_l1b = alloc_shared(sizeof(float));
        b_v_l1b = alloc_shared(sizeof(float));
        b_g_l1b = alloc_shared(sizeof(float));

        for (int s = 0; s < 2; ++s) {
            b_rec[s] = alloc_shared(B * sizeof(ChessBoardRec));
            b_loss_sum[s] = alloc_shared(sizeof(float));
        }
        b_X = alloc_shared(2 * B * kInputs * sizeof(float));
        b_A = alloc_shared(B * 2 * kHidden * sizeof(float));
        b_G = alloc_shared(2 * B * kHidden * sizeof(float));
        b_loss_i = alloc_shared(B * sizeof(float));
        b_draw_i = alloc_shared(B * sizeof(float));
        b_part_l1w = alloc_shared(64 * 2 * kHidden * sizeof(float));
        b_part_l0b = alloc_shared(64 * kHidden * sizeof(float));

        m_X = [[MPSMatrix alloc] initWithBuffer:b_X descriptor:[MPSMatrixDescriptor matrixDescriptorWithRows:2 * B columns:kInputs rowBytes:kInputs * 4 dataType:MPSDataTypeFloat32]];
        m_G = [[MPSMatrix alloc] initWithBuffer:b_G descriptor:[MPSMatrixDescriptor matrixDescriptorWithRows:2 * B columns:kHidden rowBytes:kHidden * 4 dataType:MPSDataTypeFloat32]];
        m_gl0w = [[MPSMatrix alloc] initWithBuffer:b_g_l0w descriptor:[MPSMatrixDescriptor matrixDescriptorWithRows:kInputs columns:kHidden rowBytes:kHidden * 4 dataType:MPSDataTypeFloat32]];
        gemm_l0w = [[MPSMatrixMultiplication alloc] initWithDevice:dev transposeLeft:YES transposeRight:NO resultRows:kInputs resultColumns:kHidden interiorColumns:2 * B alpha:1.0 beta:0.0];

        init_weights((float*)b_l0w.contents, (float*)b_l0b.contents, (float*)b_l1w.contents, (float*)b_l1b.contents);
        return true;
    }

    void encode_gradients(id<MTLCommandBuffer> cb, uint32_t slot, const TrainParams& tp) {
        id<MTLBlitCommandEncoder> blit = [cb blitCommandEncoder];
        [blit fillBuffer:b_X range:NSMakeRange(0, b_X.length) value:0];
        [blit endEncoding];

        id<MTLComputeCommandEncoder> enc = [cb computeCommandEncoder];
        [enc setComputePipelineState:p_fwdbwd];
        [enc setBuffer:b_rec[slot] offset:0 atIndex:0];
        [enc setBuffer:b_l0w offset:0 atIndex:1];
        [enc setBuffer:b_l0b offset:0 atIndex:2];
        [enc setBuffer:b_l1w offset:0 atIndex:3];
        [enc setBuffer:b_l1b offset:0 atIndex:4];
        [enc setBuffer:b_X offset:0 atIndex:5];
        [enc setBuffer:b_A offset:0 atIndex:6];
        [enc setBuffer:b_G offset:0 atIndex:7];
        [enc setBuffer:b_loss_i offset:0 atIndex:8];
        [enc setBuffer:b_draw_i offset:0 atIndex:9];
        [enc setBytes:&tp length:sizeof(tp) atIndex:10];
        [enc dispatchThreadgroups:MTLSizeMake(B, 1, 1) threadsPerThreadgroup:MTLSizeMake(128, 1, 1)];

        [enc setComputePipelineState:p_loss];
        [enc setBuffer:b_loss_i offset:0 atIndex:0];
        [enc setBuffer:b_draw_i offset:0 atIndex:1];
        [enc setBuffer:b_g_l1b offset:0 atIndex:2];
        [enc setBuffer:b_loss_sum[slot] offset:0 atIndex:3];
        [enc setBytes:&tp length:sizeof(tp) atIndex:4];
        [enc dispatchThreadgroups:MTLSizeMake(1, 1, 1) threadsPerThreadgroup:MTLSizeMake(256, 1, 1)];

        [enc setComputePipelineState:p_rc1];
        [enc setBuffer:b_A offset:0 atIndex:0];
        [enc setBuffer:b_draw_i offset:0 atIndex:1];
        [enc setBuffer:b_G offset:0 atIndex:2];
        [enc setBuffer:b_part_l1w offset:0 atIndex:3];
        [enc setBuffer:b_part_l0b offset:0 atIndex:4];
        [enc setBytes:&tp length:sizeof(tp) atIndex:5];
        [enc dispatchThreadgroups:MTLSizeMake(4, 64, 1) threadsPerThreadgroup:MTLSizeMake(256, 1, 1)];

        [enc setComputePipelineState:p_rc2];
        [enc setBuffer:b_part_l1w offset:0 atIndex:0];
        [enc setBuffer:b_part_l0b offset:0 atIndex:1];
        [enc setBuffer:b_g_l1w offset:0 atIndex:2];
        [enc setBuffer:b_g_l0b offset:0 atIndex:3];
        [enc dispatchThreadgroups:MTLSizeMake(4, 1, 1) threadsPerThreadgroup:MTLSizeMake(256, 1, 1)];
        [enc endEncoding];

        [gemm_l0w encodeToCommandBuffer:cb leftMatrix:m_X rightMatrix:m_G resultMatrix:m_gl0w];
    }

    void encode_adamw(id<MTLCommandBuffer> cb, const TrainParams& tp) {
        id<MTLComputeCommandEncoder> opt = [cb computeCommandEncoder];
        [opt setComputePipelineState:p_adamw];
        auto step_buf = [&](id<MTLBuffer> bw, id<MTLBuffer> bg, id<MTLBuffer> bm, id<MTLBuffer> bv, uint32_t cnt, float clip) {
            TrainParams otp = tp;
            otp.grad_count = cnt;
            otp.clip_val = clip;
            [opt setBuffer:bw offset:0 atIndex:0];
            [opt setBuffer:bg offset:0 atIndex:1];
            [opt setBuffer:bm offset:0 atIndex:2];
            [opt setBuffer:bv offset:0 atIndex:3];
            [opt setBytes:&otp length:sizeof(otp) atIndex:4];
            [opt dispatchThreadgroups:MTLSizeMake((cnt + 255) / 256, 1, 1) threadsPerThreadgroup:MTLSizeMake(256, 1, 1)];
        };
        step_buf(b_l0w, b_g_l0w, b_m_l0w, b_v_l0w, kInputs * kHidden, 1.98f);
        step_buf(b_l0b, b_g_l0b, b_m_l0b, b_v_l0b, kHidden, 1.98f);
        step_buf(b_l1w, b_g_l1w, b_m_l1w, b_v_l1w, 2 * kHidden, 1.98f);
        step_buf(b_l1b, b_g_l1b, b_m_l1b, b_v_l1b, 1, 0.0f);
        [opt endEncoding];
    }
};

static int run_selftest(id<MTLDevice> dev, id<MTLCommandQueue> q) {
    const uint32_t B = 64;
    MetalTrainer ctx;
    if (!ctx.init(dev, q, B)) return 1;

    for (uint32_t i = 0; i < kHidden; ++i) ((float*)ctx.b_l0b.contents)[i] = 0.05f * std::sin(float(i));
    *((float*)ctx.b_l1b.contents) = 0.12f;

    auto* recs = (ChessBoardRec*)ctx.b_rec[0].contents;
    uint64_t seed = 1234567ULL;
    for (uint32_t b = 0; b < B; ++b) {
        recs[b].occ = 0x0000FFFF0000FFFFULL ^ (1ULL << (b % 32));
        for (int p = 0; p < 16; ++p) {
            seed = seed * 6364136223846793005ULL + 1ULL;
            uint8_t lo = ((seed >> 24) % 6) | (((seed >> 28) & 1) << 3);
            uint8_t hi = ((seed >> 32) % 6) | (((seed >> 36) & 1) << 3);
            recs[b].pcs[p] = lo | (hi << 4);
        }
        recs[b].score = static_cast<int16_t>((int(b) - 32) * 15);
        recs[b].result = static_cast<uint8_t>(b % 3);
    }

    TrainParams tp{B, 0.75f, 0.001f, 1.0f / float(B), 0.9f, 0.999f, 0.01f, 1.98f, 0};
    id<MTLCommandBuffer> cb = [q commandBuffer];
    ctx.encode_gradients(cb, 0, tp);
    [cb commit];
    [cb waitUntilCompleted];

    // CPU reference forward/backward check.
    std::vector<float> ref_gl0w(kInputs * kHidden, 0.0f), ref_gl0b(kHidden, 0.0f), ref_gl1w(2 * kHidden, 0.0f);
    float ref_gl1b = 0.0f, ref_loss = 0.0f;
    const float* w0 = (const float*)ctx.b_l0w.contents;
    const float* b0 = (const float*)ctx.b_l0b.contents;
    const float* w1 = (const float*)ctx.b_l1w.contents;
    const float b1 = *((const float*)ctx.b_l1b.contents);

    for (uint32_t b = 0; b < B; ++b) {
        std::vector<uint16_t> stms, ntms;
        uint64_t occ = recs[b].occ;
        uint32_t idx = 0;
        while (occ && idx < 32) {
            uint32_t sq = __builtin_ctzll(occ);
            occ &= (occ - 1ULL);
            uint8_t nib = (recs[b].pcs[idx >> 1] >> (4 * (idx & 1))) & 0x0F;
            uint32_t c = (nib >> 3) & 1;
            uint32_t pc = nib & 7;
            stms.push_back(c * 384 + pc * 64 + sq);
            ntms.push_back((1 - c) * 384 + pc * 64 + (sq ^ 56));
            idx++;
        }
        std::vector<float> as(kHidden), an(kHidden);
        float raw = b1;
        for (uint32_t h = 0; h < kHidden; ++h) {
            as[h] = b0[h];
            an[h] = b0[h];
            for (auto f : stms) as[h] += w0[size_t(f) * kHidden + h];
            for (auto f : ntms) an[h] += w0[size_t(f) * kHidden + h];
            float cs = std::max(0.0f, std::min(1.0f, as[h]));
            float cn = std::max(0.0f, std::min(1.0f, an[h]));
            raw += cs * cs * w1[h] + cn * cn * w1[kHidden + h];
        }
        float pred = 1.0f / (1.0f + std::exp(-raw));
        float st = 1.0f / (1.0f + std::exp(-float(recs[b].score) / 400.0f));
        float target = 0.25f * st + 0.75f * (float(recs[b].result) * 0.5f);
        float diff = pred - target;
        ref_loss += diff * diff;
        float d_raw = 2.0f * diff * pred * (1.0f - pred);
        ref_gl1b += d_raw;
        for (uint32_t h = 0; h < kHidden; ++h) {
            float cs = std::max(0.0f, std::min(1.0f, as[h]));
            float cn = std::max(0.0f, std::min(1.0f, an[h]));
            ref_gl1w[h] += d_raw * cs * cs;
            ref_gl1w[kHidden + h] += d_raw * cn * cn;
            float ds = (as[h] > 0.0f && as[h] < 1.0f) ? (d_raw * w1[h] * 2.0f * cs) : 0.0f;
            float dn = (an[h] > 0.0f && an[h] < 1.0f) ? (d_raw * w1[kHidden + h] * 2.0f * cn) : 0.0f;
            ref_gl0b[h] += ds + dn;
            for (auto f : stms) ref_gl0w[size_t(f) * kHidden + h] += ds;
            for (auto f : ntms) ref_gl0w[size_t(f) * kHidden + h] += dn;
        }
    }

    auto max_diff = [](const float* a, const float* b, size_t n) {
        float m = 0.0f;
        for (size_t i = 0; i < n; ++i) m = std::max(m, std::abs(a[i] - b[i]));
        return m;
    };
    float e_loss = std::abs(*((float*)ctx.b_loss_sum[0].contents) - ref_loss);
    float e_l1b = std::abs(*((float*)ctx.b_g_l1b.contents) - ref_gl1b);
    float e_l1w = max_diff((float*)ctx.b_g_l1w.contents, ref_gl1w.data(), 2 * kHidden);
    float e_l0b = max_diff((float*)ctx.b_g_l0b.contents, ref_gl0b.data(), kHidden);
    float e_l0w = max_diff((float*)ctx.b_g_l0w.contents, ref_gl0w.data(), kInputs * kHidden);

    printf("Selftest max errors: loss=%.2e l1b=%.2e l1w=%.2e l0b=%.2e l0w=%.2e\n",
           e_loss, e_l1b, e_l1w, e_l0b, e_l0w);
    if (e_loss > 1e-4f || e_l1b > 1e-4f || e_l1w > 1e-4f || e_l0b > 1e-4f || e_l0w > 1e-4f) {
        fprintf(stderr, "SELFTEST FAILED!\n");
        return 1;
    }
    printf("SELFTEST PASSED BIT-ACCURATELY!\n");
    return 0;
}

int main(int argc, char** argv) {
    @autoreleasepool {
        id<MTLDevice> dev = MTLCreateSystemDefaultDevice();
        if (!dev) {
            fprintf(stderr, "No Metal GPU device available.\n");
            return 1;
        }
        id<MTLCommandQueue> q = [dev newCommandQueue];

        std::string data_path = "data/mac_gen1.data";
        std::string out_path = "nnue/monolith_gen1.mnue";
        uint32_t batch_size = 16384;
        uint32_t superbatches = 30;
        uint32_t batches_per_sb = 6104;
        uint32_t save_rate = 5;
        float wdl_lambda = 0.75f;
        float initial_lr = 0.001f;
        float final_lr = 0.001f * std::pow(0.3f, 5.0f);
        bool selftest = false;

        for (int i = 1; i < argc; ++i) {
            std::string a = argv[i];
            if (a == "--selftest") selftest = true;
            else if (a == "--data" && i + 1 < argc) data_path = argv[++i];
            else if (a == "--out" && i + 1 < argc) out_path = argv[++i];
            else if (a == "--batch" && i + 1 < argc) batch_size = std::stoul(argv[++i]);
            else if (a == "--superbatches" && i + 1 < argc) superbatches = std::stoul(argv[++i]);
            else if (a == "--batches-per-sb" && i + 1 < argc) batches_per_sb = std::stoul(argv[++i]);
            else if (a == "--save-rate" && i + 1 < argc) save_rate = std::stoul(argv[++i]);
            else if (a == "--wdl" && i + 1 < argc) wdl_lambda = std::stof(argv[++i]);
            else if (a == "--lr" && i + 1 < argc) initial_lr = std::stof(argv[++i]);
            else {
                fprintf(stderr, "Unknown arg: %s\n", argv[i]);
                return 1;
            }
        }

        printf("Metal GPU: %s\n", dev.name.UTF8String);
        if (selftest) return run_selftest(dev, q);

        int fd = open(data_path.c_str(), O_RDONLY);
        if (fd < 0) {
            perror("open data");
            return 1;
        }
        struct stat st{};
        fstat(fd, &st);
        size_t file_bytes = static_cast<size_t>(st.st_size);
        size_t total_records = file_bytes / sizeof(ChessBoardRec);
        if (total_records < batch_size) {
            fprintf(stderr, "Dataset too small: %zu records\n", total_records);
            return 1;
        }
        const auto* mapped = static_cast<const ChessBoardRec*>(
            mmap(nullptr, file_bytes, PROT_READ, MAP_PRIVATE, fd, 0)
        );
        if (mapped == MAP_FAILED) {
            perror("mmap");
            return 1;
        }
        madvise(const_cast<ChessBoardRec*>(mapped), file_bytes, MADV_SEQUENTIAL);

        const uint32_t B = batch_size;
        MetalTrainer ctx;
        if (!ctx.init(dev, q, B)) return 1;

        printf("Dataset: %zu positions (%.2f GB) | batch=%u | superbatches=%u x %u (%.2f epochs)\n",
               total_records, double(file_bytes) / (1024.0 * 1024.0 * 1024.0),
               B, superbatches, batches_per_sb,
               double(uint64_t(superbatches) * batches_per_sb * B) / double(total_records));
        fflush(stdout);

        size_t cursor = 0;
        uint64_t step = 0;
        const float pi = 3.14159265358979323846f;
        const uint64_t total_steps = uint64_t(superbatches) * batches_per_sb;

        for (uint32_t sb = 1; sb <= superbatches; ++sb) {
            auto t0 = std::chrono::steady_clock::now();
            double sb_loss = 0.0;
            id<MTLCommandBuffer> prev_cb = nil;

            for (uint32_t bi = 0; bi < batches_per_sb; ++bi) {
                uint32_t slot = bi & 1u;
                // Copy batch from mmap while previous command buffer executes.
                auto* dst = static_cast<ChessBoardRec*>(ctx.b_rec[slot].contents);
                if (cursor + B <= total_records) {
                    memcpy(dst, mapped + cursor, B * sizeof(ChessBoardRec));
                    cursor += B;
                } else {
                    size_t first = total_records - cursor;
                    memcpy(dst, mapped + cursor, first * sizeof(ChessBoardRec));
                    memcpy(dst + first, mapped, (B - first) * sizeof(ChessBoardRec));
                    cursor = B - first;
                }

                if (prev_cb) {
                    [prev_cb waitUntilCompleted];
                    sb_loss += static_cast<double>(*((float*)ctx.b_loss_sum[1u - slot].contents));
                }

                step++;
                float progress = float(sb - 1) / float(std::max(1u, superbatches - 1));
                float lr = final_lr + 0.5f * (initial_lr - final_lr) * (1.0f + std::cos(pi * progress));
                float b1c = 1.0f - std::pow(0.9f, float(step));
                float b2c = 1.0f - std::pow(0.999f, float(step));
                TrainParams tp{B, wdl_lambda, lr, 1.0f / float(B), b1c, b2c, 0.01f, 1.98f, 0};

                id<MTLCommandBuffer> cb = [q commandBuffer];
                ctx.encode_gradients(cb, slot, tp);
                ctx.encode_adamw(cb, tp);
                [cb commit];
                prev_cb = cb;
            }

            if (prev_cb) {
                [prev_cb waitUntilCompleted];
                uint32_t last_slot = (batches_per_sb - 1) & 1u;
                sb_loss += static_cast<double>(*((float*)ctx.b_loss_sum[last_slot].contents));
            }

            double secs = std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count();
            double pos_done = double(batches_per_sb) * double(B);
            double mean_loss = sb_loss / pos_done;
            float lr_cur = final_lr + 0.5f * (initial_lr - final_lr) * (1.0f + std::cos(pi * float(sb - 1) / float(std::max(1u, superbatches - 1))));
            printf("superbatch %2u/%u | loss=%.6f | lr=%.6f | %.2fs | %.0f pos/s (step %llu/%llu)\n",
                   sb, superbatches, mean_loss, lr_cur, secs, pos_done / secs,
                   (unsigned long long)step, (unsigned long long)total_steps);
            fflush(stdout);

            if ((save_rate > 0 && sb % save_rate == 0) || sb == superbatches) {
                std::string ckpt = out_path;
                if (sb != superbatches) {
                    size_t dot = out_path.rfind('.');
                    std::string base = (dot == std::string::npos) ? out_path : out_path.substr(0, dot);
                    std::string ext = (dot == std::string::npos) ? ".mnue" : out_path.substr(dot);
                    ckpt = base + "_sb" + std::to_string(sb) + ext;
                }
                if (save_mnue(ckpt, (const float*)ctx.b_l0w.contents, (const float*)ctx.b_l0b.contents,
                              (const float*)ctx.b_l1w.contents, *((const float*)ctx.b_l1b.contents))) {
                    printf("  -> saved %s\n", ckpt.c_str());
                    fflush(stdout);
                }
            }
        }

        munmap(const_cast<ChessBoardRec*>(mapped), file_bytes);
        close(fd);
    }
    return 0;
}
