"""Write the libraries docs/Performance.md measures, so the figures can be redone.

The libraries are large and are never committed, so the page names this script
instead of a file. Two shapes are produced: a ladder of three sizes drawn from a
small vocabulary, which is the shape of a real library, and one library whose
every word is unique, which is the worst case for a word table.

    python3 scripts/genlib.py /tmp/bench
    ./target/release/citerack --open-time /tmp/bench/lib100k.bib
"""

import random
import sys

VOCABULARY = (
    "neural sparse sampling manifold privacy gradient kernel lattice "
    "tensor batch decoder encoder attention residual diffusion spectral "
    "embedding alignment retrieval ranking corpus token vector prompt "
    "transformer distillation quantisation pruning scheduler checkpoint"
).split()

SURNAMES = "muller adams zhao smith doe chen wang garcia kim novak rossi".split()

SENTENCE = (
    "We present a {a} {b} method for {c} that improves {d} on {e} "
    "with a {f} {g} and a {h} {i}."
)


def ordinary(number, rng):
    """One entry with the field set and the repeated abstract of a real library."""
    pool = [rng.choice(VOCABULARY) for _ in range(40)]
    abstract = " ".join(
        SENTENCE.format(
            a=pool[i],
            b=pool[i + 1],
            c=pool[i + 2],
            d=pool[i + 3],
            e=pool[i + 4],
            f=pool[i + 5],
            g=pool[i + 6],
            h=pool[i + 7],
            i=pool[i + 8],
        )
        for i in range(0, 32, 9)
    )
    author = " and ".join(
        "%s, %s" % (rng.choice(SURNAMES).capitalize(), chr(65 + rng.randrange(26)))
        for _ in range(3)
    )
    return (
        "@article{ref%d,\n"
        "  author = {%s},\n"
        "  title  = {%s %s %s},\n"
        "  journal = {Journal of %s %s},\n"
        "  year   = {%d},\n"
        "  volume = {%d},\n"
        "  doi    = {10.1000/ref%d},\n"
        "  abstract = {%s},\n"
        "}\n\n"
    ) % (
        number,
        author,
        rng.choice(VOCABULARY).capitalize(),
        rng.choice(VOCABULARY),
        rng.choice(VOCABULARY).capitalize(),
        rng.choice(VOCABULARY).capitalize(),
        rng.choice(VOCABULARY),
        1950 + number % 75,
        number % 90 + 1,
        number,
        abstract,
    )


def unique(number, rng):
    """One entry whose every word is new, so the vocabulary is as wide as it gets."""
    letters = "abcdefghijklmnopqrstuvwxyz"
    abstract = " ".join(
        "w%d%s"
        % (
            rng.randrange(10**6),
            "".join(rng.choice(letters) for _ in range(5)),
        )
        for _ in range(20)
    )
    return (
        "@article{high%d,\n"
        "  author = {Author%d, Given},\n"
        "  title  = {Title %d %s},\n"
        "  year   = {%d},\n"
        "  abstract = {%s},\n"
        "}\n\n"
    ) % (number, number, number, abstract[:40], 1950 + number % 75, abstract)


def write(path, count, maker, seed):
    rng = random.Random(seed)
    with open(path, "w") as handle:
        for number in range(count):
            handle.write(maker(number, rng))


def main(target):
    write("%s/lib15k.bib" % target, 15000, ordinary, 1)
    write("%s/lib50k.bib" % target, 50000, ordinary, 2)
    write("%s/lib100k.bib" % target, 100000, ordinary, 3)
    write("%s/lib30k_unique.bib" % target, 30000, unique, 4)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "/tmp/bench")
