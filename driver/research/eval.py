# SPDX-License-Identifier: LGPL-2.1-or-later

"""Offline comparison of matchers on the dataset collected with elan-capture.

Each finger is enrolled with some of its touches; the others are used to
verify, against the enrolled finger (genuine) and the other fingers (impostor).

    python3 research/eval.py [dataset] [--enroll 12] [--splits 5]
"""

import argparse
import itertools
import sys
from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

PAD = 160            # FFT size: room for shifts of the 80x80 images
ANGLES = range(-30, 31, 5)
MIN_OVERLAP = 0.35   # NCC: minimum overlapping fraction of an 80x80 image


def load(dataset):
    fingers = {}
    for folder in sorted(p for p in Path(dataset).iterdir() if p.is_dir()):
        files = sorted(folder.glob("*-print.png"))
        fingers[folder.name] = [np.asarray(Image.open(f), dtype=np.float64) for f in files]
    return fingers


def preprocess(img, sigma=4.0):
    """Local contrast normalisation; the mask keeps areas touched by the finger."""
    low = ndimage.gaussian_filter(img, sigma)
    hp = img - low
    std = np.sqrt(ndimage.gaussian_filter(hp * hp, sigma)) + 1e-6
    norm = hp / std
    contrast = ndimage.gaussian_filter(std, 6)
    mask = (contrast > 0.35 * np.median(contrast)).astype(np.float64)
    mask = ndimage.binary_erosion(mask, iterations=2).astype(np.float64)
    return norm * mask, mask


def rotations(img, mask):
    out = []
    for a in ANGLES:
        if a == 0:
            out.append((img, mask))
            continue
        r = ndimage.rotate(img, a, reshape=False, order=1)
        m = ndimage.rotate(mask, a, reshape=False, order=0)
        out.append((r * m, m))
    return out


def fft(x):
    return np.fft.rfft2(x, (PAD, PAD))


def ifft(x):
    return np.fft.irfft2(x, (PAD, PAD))


class Template:
    """Enrolled touches, with the FFTs needed by the matchers."""

    def __init__(self, touches):
        self.items = []
        for img, mask in touches:
            self.items.append({
                "F": fft(img), "M": fft(mask), "F2": fft(img * img),
                "Fw": fft(img * hann_mask(mask)),
            })


_HANN = np.outer(np.hanning(80), np.hanning(80))


def hann_mask(mask):
    return _HANN * mask


def band(lo=3, hi=28):
    fy = np.fft.fftfreq(PAD)[:, None] * PAD
    fx = np.fft.rfftfreq(PAD)[None, :] * PAD
    r = np.hypot(fx, fy) * 80 / PAD   # in cycles per 80 pixels
    return ((r >= lo) & (r <= hi)).astype(np.float64)


BAND = band()


def score_poc(probe_rots, template, band_limited):
    best = 0.0
    for img, mask in probe_rots:
        G = np.conj(fft(img * hann_mask(mask)))
        for t in template.items:
            R = t["Fw"] * G
            R /= np.abs(R) + 1e-9
            if band_limited:
                R *= BAND
            c = ifft(R)
            if band_limited:
                c *= R.size / BAND.sum()
            c = ndimage.uniform_filter(c, 3, mode="wrap")  # sub-pixel peak spread
            best = max(best, float(c.max()))
    return best


def score_ncc(probe_rots, template):
    """Masked normalised cross-correlation (Padfield 2012), best over shifts with enough overlap."""
    best = -1.0
    min_n = MIN_OVERLAP * 80 * 80
    for img, mask in probe_rots:
        G, Gm, G2 = np.conj(fft(img)), np.conj(fft(mask)), np.conj(fft(img * img))
        for t in template.items:
            n = ifft(t["M"] * Gm)
            sf = ifft(t["F"] * Gm)
            sg = ifft(t["M"] * G)
            sff = ifft(t["F2"] * Gm)
            sgg = ifft(t["M"] * G2)
            sfg = ifft(t["F"] * G)
            ok = n > min_n
            if not ok.any():
                continue
            n, sf, sg, sff, sgg, sfg = (x[ok] for x in (n, sf, sg, sff, sgg, sfg))
            den = np.sqrt(np.maximum(sff - sf * sf / n, 1e-9) * np.maximum(sgg - sg * sg / n, 1e-9))
            best = max(best, float(((sfg - sf * sg / n) / den).max()))
    return best


METHODS = {
    "POC": lambda p, t: score_poc(p, t, band_limited=False),
    "BLPOC": lambda p, t: score_poc(p, t, band_limited=True),
    "NCC": score_ncc,
}


def metrics(genuine, impostor):
    genuine, impostor = np.array(genuine), np.array(impostor)
    thresholds = np.unique(np.concatenate([genuine, impostor]))
    eer, best_gap = 1.0, 2.0
    for t in thresholds:
        frr, far = (genuine < t).mean(), (impostor >= t).mean()
        if abs(frr - far) < best_gap:
            best_gap, eer = abs(frr - far), (frr + far) / 2
    # Strict threshold, like ELAN: no impostor accepted.
    strict = impostor.max()
    accepted = (genuine > strict).mean()
    return eer, accepted


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset", nargs="?", default="dataset")
    ap.add_argument("--enroll", type=int, default=12)
    ap.add_argument("--splits", type=int, default=5)
    ap.add_argument("--methods", default=",".join(METHODS), help="comma separated: " + ", ".join(METHODS))
    args = ap.parse_args()

    raw = load(args.dataset)
    prepared = {f: [preprocess(i) for i in imgs] for f, imgs in raw.items()}
    rotated = {f: [rotations(*p) for p in ps] for f, ps in prepared.items()}
    print("fingers:", {f: len(v) for f, v in raw.items()})

    rng = np.random.default_rng(0)
    methods = {m: METHODS[m] for m in args.methods.split(",")}
    results = {m: {"eer": [], "acc": [], "per": {f: [] for f in raw}, "margin": []} for m in methods}
    for split in range(args.splits):
        order = {f: (np.arange(len(v)) if split == 0 else rng.permutation(len(v))) for f, v in raw.items()}
        enrolled = {f: order[f][:args.enroll] for f in raw}
        probes = {f: order[f][args.enroll:] for f in raw}
        templates = {f: Template([prepared[f][i] for i in enrolled[f]]) for f in raw}
        for name, fn in methods.items():
            genuine, impostor, per = [], [], {f: [] for f in raw}
            for f, g in itertools.product(raw, raw):
                for i in probes[f]:
                    s = fn(rotated[f][i], templates[g])
                    (genuine if f == g else impostor).append(s)
                    if f == g:
                        per[f].append(s)
            eer, acc = metrics(genuine, impostor)
            strict = max(impostor)
            results[name]["eer"].append(eer)
            results[name]["acc"].append(acc)
            results[name]["margin"].append((np.median(genuine), strict))
            for f in raw:
                results[name]["per"][f].append((np.array(per[f]) > strict).mean())
            print(f"split {split} {name:6s} EER {eer:6.1%}  genuine accepted with 0 impostors {acc:6.1%}", file=sys.stderr)

    print(f"\n{'method':8s} {'EER':>7s} {'accepted*':>10s}  " + "  ".join(f"{f:>12s}" for f in raw))
    for name, r in results.items():
        per = "  ".join(f"{np.mean(r['per'][f]):12.0%}" for f in raw)
        print(f"{name:8s} {np.mean(r['eer']):7.1%} {np.mean(r['acc']):10.1%}  {per}")
    print("* genuine attempts accepted at the strictest threshold that rejects every impostor")


if __name__ == "__main__":
    main()
