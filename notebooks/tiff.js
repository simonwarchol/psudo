const url = 'https://lin-2023-orion-crc.s3.amazonaws.com/data/CRC40/19510_P37-S83_C40_US_SCAN_OR_001__163912-registered.ome.tif';
const start = 8;
const end = 11;

fetch(url, {
    headers: {
        'Range': `bytes=${start}-${end}`
    }
})
    .then(response => response.arrayBuffer())
    .then(buffer => {
        const bytes = new Uint8Array(buffer);
        console.log(bytes);
    });
