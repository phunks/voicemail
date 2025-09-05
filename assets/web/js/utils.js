/**
 * https://github.com/feige05/g711.js.git
 *
 * 编码wav，一般wav格式是在pcm文件前增加44个字节的文件头，
 * 所以，此处只需要在pcm数据前增加下就行了。
 * WAVエンコードでは、一般的なWAVフォーマットはPCMファイルの前に44バイトのファイルヘッダーを追加します。
 * したがって、ここではPCMデータの前に追加するだけで十分です。
 * @param {DataView} bytes       pcm二进制数据 PCMバイナリデータ
 * @param {number}  sampleRate   输入采样率 入力サンプリングレート
 * @param {number}  numChannels  声道数 チャンネル数
 * @param {number}  sampleBits   输出采样位数 出力サンプリングビット数
 * @param {boolean} littleEndian 是否是小端字节序 リトルエンディアン
 * @returns {ArrayBuffer}        wav二进制数据 WAVバイナリデータ
 */
function encodeWAV(
  bytes,
  sampleRate = 44100,
  numChannels = 2,
  sampleBits = 16,
  littleEndian = true
) {
  let buffer = new ArrayBuffer(44 + bytes.byteLength),
    data = new DataView(buffer),
    offset = 0

  // 资源交换文件标识符
  // リソース交換ファイル識別子
  writeString(data, offset, "RIFF")
  offset += 4
  // 下个地址开始到文件尾总字节数
  // 次のアドレスからファイル末尾までの総バイト数
  data.setUint32(offset, 44 + bytes.byteLength, littleEndian)
  offset += 4
  // WAV文件标志
  // WAVファイルフラグ
  writeString(data, offset, "WAVE")
  offset += 4
  // 波形格式标志
  // 波形フォーマットフラグ
  writeString(data, offset, "fmt ")
  offset += 4
  // 过滤字节,一般为 0x10 = 16
  // フィルタリングバイト、通常は 0x10 = 16
  data.setUint32(offset, 16, littleEndian)
  offset += 4
  // 格式类别 (PCM形式采样数据)
  // フォーマットカテゴリ (PCM形式サンプリングデータ)
  data.setUint16(offset, 1, littleEndian)
  offset += 2
  // 声道数
  // チャンネル数
  data.setUint16(offset, numChannels, littleEndian)
  offset += 2
  // 采样率,每秒样本数,表示每个通道的播放速度
  // サンプリングレート、サンプルレート、各チャンネルの再生速度を示す
  data.setUint32(offset, sampleRate, littleEndian)
  offset += 4
  // 波形数据传输率 (每秒平均字节数) 声道数 × 采样频率 × 采样位数 / 8
  // 波形データ転送速度（平均バイト/秒）チャンネル数 × サンプリング周波数 × サンプリングビット数 / 8
  data.setUint32(
    offset,
    numChannels * sampleRate * (sampleBits / 8),
    littleEndian
  )
  offset += 4
  // 快数据调整数 采样一次占用字节数 声道数 × 采样位数 / 8
  // クイックデータ調整数1回のサンプリングで占有するバイト数チャンネル数 × サンプリングビット数 / 8
  data.setUint16(offset, numChannels * (sampleBits / 8), littleEndian)
  offset += 2
  // 采样位数
  // データ識別子
  data.setUint16(offset, sampleBits, littleEndian)
  offset += 2
  // 数据标识符
  writeString(data, offset, "data")
  offset += 4
  // 采样数据总数,即数据总大小-44
  // サンプリングデータ総数、データ総サイズ-44
  data.setUint32(offset, bytes.byteLength, littleEndian)
  offset += 4

  // 给wav头增加pcm体
  // WAVヘッダーにPCMボディを追加する
  for (let i = 0; i < bytes.byteLength; ) {
    data.setUint8(offset, bytes.getUint8(i))
    offset++
    i++
  }

  return buffer
}

function writeString(view, offset, string) {
  for (let i = 0, len = string.length; i < len; i++) {
    view.setUint8(offset + i, string.charCodeAt(i))
  }
}
