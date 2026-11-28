#!/usr/bin/env python3
"""Annotate a call site in a PE image.

Reverse engineering XAML by hand meant the same lookups every time: which import a
`call *0x...(%%rip)` goes through, and whether a `lea` points at a GUID. This wraps the
disassembler that is already installed and adds those answers to the listing.

usage: annotate.py <image> <rva> [instructions]
"""
import struct
import subprocess
import sys
import uuid


def load(path):
    data = open(path, 'rb').read()
    pe = struct.unpack_from('<I', data, 0x3C)[0]
    magic = struct.unpack_from('<H', data, pe + 24)[0]
    count = struct.unpack_from('<H', data, pe + 6)[0]
    opt_size = struct.unpack_from('<H', data, pe + 20)[0]
    if magic == 0x20B:
        base = struct.unpack_from('<Q', data, pe + 48)[0]
        directories = pe + 24 + 112
    else:
        base = struct.unpack_from('<I', data, pe + 52)[0]
        directories = pe + 24 + 96
    table = []
    offset = pe + 24 + opt_size
    for index in range(count):
        entry = offset + index * 40
        virtual_size, address, raw_size, raw = struct.unpack_from('<IIII', data, entry + 8)
        name = data[entry:entry + 8].split(b'\0')[0].decode('latin-1')
        table.append((name, address, virtual_size, raw, raw_size))
    return data, base, table, directories


def locate(table, rva):
    for name, address, virtual_size, raw, raw_size in table:
        if address <= rva < address + max(virtual_size, raw_size):
            return raw + (rva - address)
    return None


def import_names(data, base, table, directories):
    names = {}
    rva, size = struct.unpack_from('<II', data, directories + 8)
    offset = locate(table, rva)
    if offset is None:
        return names
    while True:
        original, stamp, chain, module_rva, first = struct.unpack_from('<IIIII', data, offset)
        if not module_rva:
            break
        module_offset = locate(table, module_rva)
        module = data[module_offset:data.index(b'\0', module_offset)].decode('latin-1')
        thunk = locate(table, original or first)
        slot = 0
        while thunk is not None:
            value = struct.unpack_from('<Q', data, thunk + slot * 8)[0]
            if not value:
                break
            if not value & (1 << 63):
                name_offset = locate(table, value & 0x7FFFFFFF)
                if name_offset is not None:
                    name = data[name_offset + 2:data.index(b'\0', name_offset + 2)].decode('latin-1')
                    names[base + first + slot * 8] = module + '!' + name
            slot += 1
        offset += 20
    return names


def codeview(data, table):
    """Return (pdb name, key) from the debug directory, where key is GUID+age."""
    pe = struct.unpack_from('<I', data, 0x3C)[0]
    magic = struct.unpack_from('<H', data, pe + 24)[0]
    directories = pe + 24 + (112 if magic == 0x20B else 96)
    rva, size = struct.unpack_from('<II', data, directories + 48)
    offset = locate(table, rva)
    if offset is None:
        return None
    for index in range(size // 28):
        entry = offset + index * 28
        kind = struct.unpack_from('<I', data, entry + 12)[0]
        length = struct.unpack_from('<I', data, entry + 16)[0]
        raw = struct.unpack_from('<I', data, entry + 24)[0]
        if kind != 2 or data[raw:raw + 4] != b'RSDS':
            continue
        guid = data[raw + 4:raw + 20]
        age = struct.unpack_from('<I', data, raw + 20)[0]
        name = data[raw + 24:raw + 24 + length].split(b'\0')[0].decode('latin-1')
        return name, guid.hex().upper() + '%X' % age
    return None


def annotate(path, rva, count):
    data, base, table, directories = load(path)
    imports = import_names(data, base, table, directories)
    start = base + rva
    listing = subprocess.run(
        ['x86_64-w64-mingw32-objdump', '-d', '--start-address=%#x' % start,
         '--stop-address=%#x' % (start + count * 8), path],
        capture_output=True, text=True).stdout
    for line in listing.splitlines():
        note = ''
        if '#' in line:
            field = line.rsplit('#', 1)[1].split()
            if field:
                try:
                    target = int(field[0], 16)
                except ValueError:
                    target = None
                if target in imports:
                    note = '   <- ' + imports[target]
                elif target and 'lea' in line:
                    offset = locate(table, target - base)
                    if offset is not None and offset + 16 <= len(data):
                        raw = data[offset:offset + 16]
                        if raw[8:10] != b'\0\0' and raw[0:2] not in (b'\0\0', b'MM'):
                            note = '   <- GUID ' + str(uuid.UUID(bytes_le=raw))
        print(line + note)


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    data, base, table, directories = load(sys.argv[1])
    if sys.argv[2] == '--pdb':
        found = codeview(data, table)
        if not found:
            print('no CodeView record')
            return 1
        name, key = found
        base_name = name.replace('\\', '/').rsplit('/', 1)[-1]
        print(name)
        print(key)
        print('https://msdl.microsoft.com/download/symbols/%s/%s/%s' % (base_name, key, base_name))
        return 0
    annotate(sys.argv[1], int(sys.argv[2], 0), int(sys.argv[3]) if len(sys.argv) > 3 else 24)
    return 0


if __name__ == '__main__':
    sys.exit(main())