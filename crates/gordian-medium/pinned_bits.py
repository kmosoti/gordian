"""Independent computation of the pinned bits in crates/gordian-medium/src/oscillome.rs.

The same series as exp_det, in IEEE double precision (Python floats), with Rust's round (half
away from zero), then rounded to single precision with struct.pack. Cross-checked against
math.exp.
"""
import math
import struct

LN2 = math.log(2)
assert LN2.hex() == "0x1.62e42fefa39efp-1", LN2.hex()


def exp_det(x):
    x = min(max(x, -700.0), 700.0)
    k = x / LN2
    k = math.floor(k + 0.5) if k >= 0 else -math.floor(-k + 0.5)
    r = x - k * LN2
    term = 1.0
    s = 1.0
    for i in range(1, 30):
        term *= r / float(i)
        s += term
    scale = struct.unpack("<d", struct.pack("<Q", (int(k) + 1023) << 52))[0]
    return s * scale


def b64(v):
    return hex(struct.unpack("<Q", struct.pack("<d", v))[0])


def b32(v):
    return hex(struct.unpack("<I", struct.pack("<f", v))[0])


print("exp(1)", b64(exp_det(1.0)), "exp(-3.7)", b64(exp_det(-3.7)))
print("exp(-0.1)", b64(exp_det(-0.1)), "math", b64(math.exp(-0.1)))
for tau, tick in [(1.0, 0.1), (1.0, 0.5), (1.0, 2.0), (10.0, 2.0)]:
    x = -(round(tick * 1e9)) / (round(tau * 1e9))
    print(f"decay tau={tau} tick={tick}", b32(exp_det(x)), "math", b32(math.exp(x)))
x = -1e8 / 1e10
print("rate tau=10 tick=0.1", b32(1.0 - exp_det(x)), "math", b32(1.0 - math.exp(x)))
