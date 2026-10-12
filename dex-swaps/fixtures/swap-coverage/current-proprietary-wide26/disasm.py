import struct,pathlib
for p in pathlib.Path('/Users/jordynnitch/code/pluto/.indexer-work/audit26-proprietary').glob('*.elf'):
 b=p.read_bytes();h=struct.unpack_from('<16sHHIQQQIHHHHHH',b); sh=[struct.unpack_from('<IIQQQQIIQQ',b,h[6]+i*h[11]) for i in range(h[12])]; names=b[sh[h[13]][4]:sh[h[13]][4]+sh[h[13]][5]];text=next(s for s in sh if names[s[0]:].split(b'\0')[0]==b'.text'); code=b[text[4]:text[4]+text[5]]
 rows=[]
 for i in range(0,len(code),8):
  op,reg,off,imm=struct.unpack_from('<BBhi',code,i);dst=reg&15;src=reg>>4;pc=text[3]+i; idx=i//8
  if op==0x18: val=f'r{dst}=LDDW {imm&0xffffffff:08x}'
  elif op==0x85: val=f'call {imm} target={idx+1+imm}'
  elif op==0x95: val='exit'
  elif op&7 in (5,6): val=f'jump {hex(op)} r{dst}, '+(f'r{src}' if op&8 else str(imm))+f' -> {idx+off+1}'
  elif op&7 in (4,7): val=f'ALU {hex(op>>4)} r{dst}, '+(f'r{src}' if op&8 else str(imm))
  elif op&7==1: val=f'r{dst}=LOAD{hex(op)} [r{src}+{off}]'
  elif op&7 in (2,3): val=f'STORE{hex(op)} [r{dst}+{off}], '+(f'r{src}' if op&7==3 else str(imm))
  else: val=f'{op:02x} r{dst} r{src} off={off} imm={imm}'
  rows.append(f'{idx:5} {pc:08x} {val}')
 p.with_suffix('.disasm.txt').write_text('\n'.join(rows)+'\n')
