import os, struct
ico = os.path.join(os.path.dirname(__file__), '..', 'pc', 'windows-server', 'assets', 'vrc.ico')
data = open(ico, 'rb').read()
reserved, typ, count = struct.unpack_from('<HHH', data, 0)
for i in range(count):
    e = 6 + i * 16
    w, h = data[e], data[e + 1]
    planes, bpp = struct.unpack_from('<HH', data, e + 4)
    size, off = struct.unpack_from('<II', data, e + 8)
    sig = data[off:off + 4]
    print(f'frame w={w or 256} bpp={bpp} size={size} png={sig == bytes([0x89, 0x50, 0x4E, 0x47])}')
