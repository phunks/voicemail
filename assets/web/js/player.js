const audioPlayers = new Map();

async function togglePlay(id, btn) {
    const player = audioPlayers.get(id);

    if (player && player.playing) {
        player.source.stop();
        player.ctx.close();
        audioPlayers.set(id, { playing: false });
        btn.src = 'img/play.svg';
        return;
    }

    const audioUrl = `/api/voice/${id}`;
    const res = await fetch(audioUrl);
    const arrayBuffer = await res.arrayBuffer();
    const i16A = ulawToPCM(new Uint8Array(arrayBuffer), 16);
    const wavBuf = encodeWAV(new DataView(i16A.buffer), 8000, 1, 16);
    const ctx = new (window.AudioContext || window.webkitAudioContext)();

    const audioBuffer = await ctx.decodeAudioData(wavBuf);
    const source = ctx.createBufferSource();
    source.buffer = audioBuffer;
    source.connect(ctx.destination);

    source.start(0);
    btn.src = 'img/stop.svg';

    source.onended = () => {
        audioPlayers.set(id, { playing: false });
        btn.src = 'img/play.svg';
    };

    audioPlayers.set(id, { ctx, source, playing: true });
}