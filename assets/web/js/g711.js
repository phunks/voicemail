
/**
 * G711U(u-law) - PCM converter
 * https://github.com/feige05/g711.js.git
 */
const SIGN_BIT = 0x80
const QUANT_MASK = 0x0f
const SEG_SHIFT = 0x04
const SEG_MASK = 0x70

const BIAS = 0x84

// const seg_end = new Uint16Array([0x1F, 0x3F, 0x7F, 0xFF, 0x1FF, 0x3FF, 0x7FF, 0xFFF]);
const seg_end = new Uint16Array([
    0xff,
    0x1ff,
    0x3ff,
    0x7ff,
    0xfff,
    0x1fff,
    0x3fff,
    0x7fff
])
// const seg_uend = new Uint16Array([0x3F, 0x7F, 0xFF, 0x1FF, 0x3FF, 0x7FF, 0xFFF, 0x1FFF]);

/* 16384 entries per table (8 bit) */
const ulaw_to_linear = new Array(256)

// 初始化ulaw表
for (let i = 0; i < 256; i++) ulaw_to_linear[i] = ulaw2linear(i)

function ulaw2linear(u_val) {
    let t
    u_val = ~u_val
    t = ((u_val & QUANT_MASK) << 3) + BIAS
    t <<= (u_val & SEG_MASK) >>> SEG_SHIFT

    return (u_val & SIGN_BIT) > 0 ? BIAS - t : t - BIAS
}

function linear2ulaw(pcm_val) {
    let mask
    let seg
    let uval

    /* Get the sign and the magnitude of the value. */
    if (pcm_val < 0) {
        pcm_val = BIAS - pcm_val
        mask = 0x7f
    } else {
        pcm_val += BIAS
        mask = 0xff
    }

    /* Convert the scaled magnitude to segment number. */
    seg = search(pcm_val, seg_end, 8)

    /*
     * Combine the sign, segment, quantization bits;
     * and complement the code word.
     */
    if (seg >= 8)
        /* out of range, return maximum value. */
        return 0x7f ^ mask
    else {
        uval = (seg << 4) | ((pcm_val >> (seg + 3)) & 0xf)
        return uval ^ mask
    }
}

function search(val, table, size) {
    for (let i = 0; i < size; i++) {
        if (val <= table[i]) return i
    }
    return size
}

function toShortArray(src) {
    let dst = new Int16Array(src.length / 2)
    for (let i = 0, k = 0; i < src.length; ) {
        dst[k++] = (src[i++] & 0xff) | ((src[i++] & 0xff) << 8)
    }
    return dst
}

/**
 *
 * @param {Int8Array} data
 * @returns
 */
function ulawToPCM(data, bit = 16) {
    let typedArray = bit === 16 ? Int16Array : Int8Array
    let dest = new typedArray(data.length)
    for (let i = 0, k = 0, len = data.length; i < len; i++) {
        dest[k++] = ulaw_to_linear[data[i] & 0xff]
    }
    return dest
}

/**
 *
 * @param { Int16Array or Int8Array } data
 */
function ulawFromPCM(data) {
    let dest = new Uint8Array(data.length)
    for (let i = 0, k = 0; i < data.length; i++) {
        dest[k++] = linear2ulaw(data[i])
    }
    return dest
}

async function playVoiceData(id) {
    const audioUrl = `/api/voice/${id}`; // 8KHZ 单声道 16-bit
    const res = await fetch(audioUrl);
    const arrayBuffer = await res.arrayBuffer(); // byte array字节数组
    const i16A = ulawToPCM(new Uint8Array(arrayBuffer),16)
    const wavBuf = encodeWAV(new DataView(i16A.buffer), 8000, 1, 16) // 8KHZ 单声道 16-bit
    const ctx = new (window.AudioContext || window.webkitAudioContext())();
    const audioBuffer = await ctx.decodeAudioData(wavBuf, decodeData => decodeData, err => console.error(err));
    const source = ctx.createBufferSource()
    source.buffer = audioBuffer; // 设置数据
    source.loop = false; //设置，循环播放
    source.connect(ctx.destination); // 头尾相连
    source.start(0); //立即播放
}
