import pathlib,struct,sys
p=pathlib.Path(sys.argv[1]) if len(sys.argv)>1 else pathlib.Path(__file__).with_name('CQwWoJENUtKmwCMqnyGbEYkg41oxdat23kkNdJLvY7v9.elf')
b=p.read_bytes();h=struct.unpack_from('<16sHHIQQQIHHHHHH',b);assert h[7] in (0,2,3)
sections=[struct.unpack_from('<IIQQQQIIQQ',b,h[6]+i*h[11]) for i in range(h[12])]
names=b[sections[h[13]][4]:sections[h[13]][4]+sections[h[13]][5]]
t=next(s for s in sections if names[s[0]:].split(b'\0')[0]==b'.text');code=b[t[4]:t[4]+t[5]];rows=[]
for i in range(0,len(code),8):
 op,reg,off,imm=struct.unpack_from('<BBhi',code,i);dst=reg&15;src=reg>>4;idx=i//8
 if op==0x18:
  high=struct.unpack_from('<I',code,i+12)[0];n=(imm&0xffffffff)|(high<<32);val=f'r{dst}=LDDW {n:#x}'
 elif op==0x85:val=f'call {imm} target={idx+1+imm}'
 elif op==0x95:val='exit'
 elif op&7 in (5,6):val=f'jump {hex(op)} r{dst}, '+(f'r{src}' if op&8 else str(imm))+f' -> {idx+off+1}'
 elif op&7 in (4,7):val=f'ALU {hex(op>>4)} r{dst}, '+(f'r{src}' if op&8 else str(imm))
 elif op&7==1:val=f'r{dst}=LOAD{ {0x61:4,0x69:2,0x71:1,0x79:8}.get(op,hex(op))} [r{src}+{off}]'
 elif op&7 in (2,3):val=f'STORE{ {0x62:4,0x6a:2,0x72:1,0x7a:8,0x63:4,0x6b:2,0x73:1,0x7b:8}.get(op,hex(op))} [r{dst}+{off}], '+(f'r{src}' if op&7==3 else str(imm))
 else:val=f'{op:02x} r{dst} r{src} off={off} imm={imm}'
 rows.append(f'{idx:6} {t[3]+i:08x} {val}')
p.with_suffix('.disasm.txt').write_text('\n'.join(rows)+'\n')
