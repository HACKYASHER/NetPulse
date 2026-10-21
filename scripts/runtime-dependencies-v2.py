"""Inspect PE runtime imports without executing or loading the examined files."""
from pathlib import Path
import json
import struct
import sys


def imports(path):
    data = Path(path).read_bytes()
    if data[:2] != b'MZ':
        raise ValueError(f'Not a Windows executable: {path}')
    pe = struct.unpack_from('<I', data, 0x3c)[0]
    if data[pe:pe + 4] != b'PE\0\0':
        raise ValueError(f'Invalid PE header: {path}')
    machine, count = struct.unpack_from('<HH', data, pe + 4)
    optional = pe + 24
    optional_size = struct.unpack_from('<H', data, pe + 20)[0]
    magic = struct.unpack_from('<H', data, optional)[0]
    if magic not in (0x10b, 0x20b):
        raise ValueError('Unknown PE optional header')
    directories = optional + (112 if magic == 0x20b else 96)
    image_base = struct.unpack_from('<Q' if magic == 0x20b else '<I', data, optional + (24 if magic == 0x20b else 28))[0]
    sections = []
    for n in range(count):
        start = optional + optional_size + n * 40
        size, rva, raw_size, offset = struct.unpack_from('<IIII', data, start + 8)
        sections.append((rva, max(size, raw_size), offset))

    def address(rva):
        for base, size, offset in sections:
            if base <= rva < base + size and offset + rva - base < len(data):
                return offset + rva - base
        raise ValueError(f'PE address out of file: {rva:x}')

    def name(rva):
        start = address(rva)
        return data[start:data.index(b'\0', start)].decode('ascii').lower()

    names = set()
    for index, stride in [(1, 20), (13, 32)]:
        rva, size = struct.unpack_from('<II', data, directories + index * 8)
        if not rva or not size:
            continue
        start = address(rva)
        for offset in range(start, min(start + size, len(data)), stride):
            fields = struct.unpack_from('<' + 'I' * (stride // 4), data, offset)
            if not any(fields):
                break
            if index == 1:
                names.add(name(fields[3]))
            else:
                names.add(name(fields[1] if fields[0] & 1 else fields[1] - image_base))
    return {'file': Path(path).name, 'machine': hex(machine), 'imports': sorted(names),
            'external_vc_runtime': sorted(n for n in names if n.startswith(('vcruntime', 'msvcp', 'concrt', 'vcomp')))}


def check(paths):
    result = [imports(path) for path in paths]
    for entry in result:
        if entry['machine'] != '0x8664':
            raise ValueError(f"Expected x64: {entry['file']}")
        if entry['external_vc_runtime']:
            raise ValueError(f"Unbundled Visual C++ dependency: {entry['file']}: {entry['external_vc_runtime']}")
    return result


if __name__ == '__main__':
    print(json.dumps(check(sys.argv[1:]), indent=2))
