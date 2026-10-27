# SPDX-License-Identifier: LGPL-2.1-or-later

"""Writes the NCC scores of split 0 (first N touches enrolled), to check the Rust matcher.

    python3 research/reference.py [dataset] [--enroll 12] > research/reference-scores.csv
"""

import argparse
import sys

sys.path.insert(0, __file__.rsplit("/", 1)[0])
import eval as ev  # noqa: E402

ap = argparse.ArgumentParser()
ap.add_argument("dataset", nargs="?", default="dataset")
ap.add_argument("--enroll", type=int, default=12)
args = ap.parse_args()

raw = ev.load(args.dataset)
prepared = {f: [ev.preprocess(i) for i in imgs] for f, imgs in raw.items()}
templates = {f: ev.Template(ps[:args.enroll]) for f, ps in prepared.items()}
print("probe_finger,probe,template_finger,score")
for f, ps in prepared.items():
    for i in range(args.enroll, len(ps)):
        rots = ev.rotations(*ps[i])
        for g, t in templates.items():
            print(f"{f},{i + 1},{g},{ev.score_ncc(rots, t):.6f}", flush=True)
