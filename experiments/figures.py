#!/usr/bin/env python3
"""Draws the figures from the CSVs in experiments/results. Standard library only.

    python3 experiments/figures.py

Writes experiments/results/crossing_error.svg: the median distance error of each
way of crossing against the number of seams crossed, on logarithmic axes.
"""

import csv
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))
RESULTS = os.path.join(HERE, "results")

# Series in fixed order: name in the CSV, label, colour, dash pattern, marker.
# Colours are the first five categorical slots of the palette the charts use;
# dash and marker repeat the identity for print and for colour-blind readers.
SERIES = [
    ("engine", "permute, counted steps (the engine)", "#2a78d6", "", "circle"),
    ("perm-run", "permute, running total", "#eb6834", "", "square"),
    ("recompute", "recompute, integer voxel", "#1baf7a", "6 3", "triangle"),
    ("fishpye", "recompute, fishpye", "#eda100", "2 3", "diamond"),
    ("cell-matrix", "matrix per cell (HyperRogue)", "#e87ba4", "8 2 2 2", "pentagon"),
]

INK = "#1f1f1e"
MUTED = "#6b6a63"
GRID = "#e4e3dd"
SURFACE = "#fcfcfb"

W, H = 720, 420
LEFT, RIGHT, TOP, BOTTOM = 64, 232, 28, 52
X_MIN, X_MAX = 1, 1000
Y_MIN, Y_MAX = 1e-7, 1.0


def x_of(crossings):
    span = math.log10(X_MAX) - math.log10(X_MIN)
    return LEFT + (math.log10(crossings) - math.log10(X_MIN)) / span * (W - LEFT - RIGHT)


def y_of(error):
    span = math.log10(Y_MAX) - math.log10(Y_MIN)
    return H - BOTTOM - (math.log10(error) - math.log10(Y_MIN)) / span * (H - TOP - BOTTOM)


def marker(kind, x, y, colour, title):
    r = 4
    if kind == "circle":
        shape = f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r}"'
    elif kind == "square":
        shape = f'<rect x="{x - r:.1f}" y="{y - r:.1f}" width="{2 * r}" height="{2 * r}"'
    elif kind == "triangle":
        pts = f"{x:.1f},{y - r - 1:.1f} {x - r - 1:.1f},{y + r:.1f} {x + r + 1:.1f},{y + r:.1f}"
        shape = f'<polygon points="{pts}"'
    elif kind == "pentagon":
        pts = " ".join(
            f"{x + (r + 1) * math.sin(k * 2 * math.pi / 5):.1f},{y - (r + 1) * math.cos(k * 2 * math.pi / 5):.1f}"
            for k in range(5)
        )
        shape = f'<polygon points="{pts}"'
    else:
        pts = f"{x:.1f},{y - r - 1:.1f} {x + r + 1:.1f},{y:.1f} {x:.1f},{y + r + 1:.1f} {x - r - 1:.1f},{y:.1f}"
        shape = f'<polygon points="{pts}"'
    # A ring in the surface colour keeps overlapping markers apart.
    return f'{shape} fill="{colour}" stroke="{SURFACE}" stroke-width="2"><title>{title}</title></{shape[1:].split()[0]}>'


def power(exponent):
    return f'10<tspan dy="-5" font-size="9">{exponent}</tspan><tspan dy="5"> </tspan>'


def crossing_error():
    rows = {}
    with open(os.path.join(RESULTS, "crossing_error.csv")) as f:
        for row in csv.DictReader(f):
            if int(row["crossings"]) >= 1:
                rows.setdefault(row["method"], []).append(
                    (int(row["crossings"]), float(row["err_median"]))
                )

    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" '
        f'font-family="Helvetica, Arial, sans-serif" font-size="12">',
        f'<rect width="{W}" height="{H}" fill="{SURFACE}"/>',
    ]

    # Grid and axes: recessive.
    for e in range(-7, 1):
        y = y_of(10.0**e)
        out.append(f'<line x1="{LEFT}" x2="{W - RIGHT}" y1="{y:.1f}" y2="{y:.1f}" stroke="{GRID}"/>')
        out.append(
            f'<text x="{LEFT - 8}" y="{y + 4:.1f}" text-anchor="end" fill="{MUTED}">{power(e)}</text>'
        )
    for c in (1, 10, 100, 1000):
        x = x_of(c)
        out.append(f'<line x1="{x:.1f}" x2="{x:.1f}" y1="{TOP}" y2="{H - BOTTOM}" stroke="{GRID}"/>')
        out.append(
            f'<text x="{x:.1f}" y="{H - BOTTOM + 18}" text-anchor="middle" fill="{MUTED}">{c:,}</text>'
        )
    out.append(
        f'<text x="{(LEFT + W - RIGHT) / 2:.1f}" y="{H - 12}" text-anchor="middle" fill="{INK}">'
        "seams crossed before the hit</text>"
    )
    out.append(
        f'<text transform="translate(16 {(TOP + H - BOTTOM) / 2:.1f}) rotate(-90)" '
        f'text-anchor="middle" fill="{INK}">median distance error (voxels)</text>'
    )

    # Lines, then markers, then a label at the end of each line.
    labels = []
    for name, label, colour, dash, kind in SERIES:
        pts = sorted(rows[name])
        path = " ".join(
            f'{"M" if i == 0 else "L"}{x_of(c):.1f},{y_of(e):.1f}' for i, (c, e) in enumerate(pts)
        )
        dashed = f' stroke-dasharray="{dash}"' if dash else ""
        out.append(f'<path d="{path}" fill="none" stroke="{colour}" stroke-width="2"{dashed}/>')
        for c, e in pts:
            title = f"{label}: {e:.2e} voxels at {c:,} crossings"
            out.append(marker(kind, x_of(c), y_of(e), colour, title))
        labels.append([y_of(pts[-1][1]), label, colour, kind])

    # Keep the end labels at least 16 px apart, in their vertical order.
    labels.sort()
    for i in range(1, len(labels)):
        labels[i][0] = max(labels[i][0], labels[i - 1][0] + 16)
    x = W - RIGHT + 12
    for y, label, colour, kind in labels:
        out.append(marker(kind, x + 4, y, colour, label))
        out.append(f'<text x="{x + 14}" y="{y + 4:.1f}" fill="{INK}">{label}</text>')

    out.append("</svg>")
    path = os.path.join(RESULTS, "crossing_error.svg")
    with open(path, "w") as f:
        f.write("\n".join(out) + "\n")
    print("wrote", os.path.relpath(path))


def crossing_state():
    """The stepping state before and after a seam that turns a quarter about z.

    The turn sends +x to +y and +y to -x (`Orientation::quarter_turn(2, 1)`),
    so the new y axis reads the old x axis, and the new x axis reads the old
    y axis with its step negated. A row keeps its colour and its rank, which
    is the axis it was in the frame the ray started in.
    """
    w, h = 720, 330
    rank_colour = ["#2a78d6", "#eb6834", "#1baf7a"]
    # (axis, step, t_max, t_delta, rank)
    before = [("x", "+1", "a", "α", 0), ("y", "−1", "b", "β", 1), ("z", "+1", "c", "γ", 2)]
    after = [("x", "+1", "b", "β", 1), ("y", "+1", "a", "α", 0), ("z", "+1", "c", "γ", 2)]
    cols = [("axis", 0), ("step", 52), ("tMax", 104), ("tDelta", 160), ("rank", 222)]
    row_h, table_w = 30, 262

    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" '
        f'font-family="Helvetica, Arial, sans-serif" font-size="13">',
        f'<rect width="{w}" height="{h}" fill="{SURFACE}"/>',
        "<defs>"
        f'<marker id="tip" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" '
        f'orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" fill="{INK}"/></marker>'
        f'<marker id="soft" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" '
        f'orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" fill="{MUTED}"/></marker>'
        "</defs>",
    ]

    def frame(x0, title, ray):
        """Axes of one room's frame, seen down z, with the ray drawn in it."""
        cx, cy, n = x0 + 56, 96, 44
        out.append(f'<text x="{x0}" y="24" fill="{INK}" font-weight="bold">{title}</text>')
        out.append(
            f'<line x1="{cx}" y1="{cy}" x2="{cx + n}" y2="{cy}" stroke="{MUTED}" marker-end="url(#soft)"/>'
        )
        out.append(
            f'<line x1="{cx}" y1="{cy}" x2="{cx}" y2="{cy - n}" stroke="{MUTED}" marker-end="url(#soft)"/>'
        )
        out.append(f'<text x="{cx + n + 6}" y="{cy + 4}" fill="{MUTED}">x</text>')
        out.append(f'<text x="{cx - 4}" y="{cy - n - 6}" fill="{MUTED}">y</text>')
        dx, dy = ray
        out.append(
            f'<line x1="{cx + 100}" y1="{cy - 14}" x2="{cx + 100 + dx}" y2="{cy - 14 - dy}" '
            f'stroke="{INK}" stroke-width="2" marker-end="url(#tip)"/>'
        )
        out.append(f'<text x="{cx + 100 + dx + 8}" y="{cy - 14 - dy + 4}" fill="{INK}">ray</text>')

    def table(x0, rows):
        y0 = 150
        for name, dx in cols:
            out.append(f'<text x="{x0 + dx + 12}" y="{y0}" fill="{MUTED}">{name}</text>')
        ys = {}
        for i, (axis, step, t_max, t_delta, rank) in enumerate(rows):
            y = y0 + 12 + i * (row_h + 6)
            colour = rank_colour[rank]
            out.append(
                f'<rect x="{x0}" y="{y}" width="{table_w}" height="{row_h}" rx="4" '
                f'fill="{SURFACE}" stroke="{colour}" stroke-width="2"/>'
            )
            out.append(f'<rect x="{x0}" y="{y}" width="6" height="{row_h}" rx="2" fill="{colour}"/>')
            for value, (_, dx) in zip((axis, step, t_max, t_delta, str(rank)), cols):
                out.append(f'<text x="{x0 + dx + 12}" y="{y + 20}" fill="{INK}">{value}</text>')
            ys[rank] = y + row_h / 2
        return ys

    left, right = 28, 430
    # The same ray in the two frames: (+, -) before, and after the turn (+, +).
    frame(left, "Before the seam", (44, -26))
    frame(right, "After the seam", (26, 44))
    y_before = table(left, before)
    y_after = table(right, after)

    # Each row goes to the axis the turn sends its axis to.
    for rank in range(3):
        x1, x2 = left + table_w + 8, right - 10
        out.append(
            f'<path d="M{x1},{y_before[rank]:.1f} C{x1 + 50},{y_before[rank]:.1f} '
            f'{x2 - 50},{y_after[rank]:.1f} {x2},{y_after[rank]:.1f}" fill="none" '
            f'stroke="{rank_colour[rank]}" stroke-width="2" marker-end="url(#soft)"/>'
        )
    mid = (left + table_w + right) / 2
    out.append(
        f'<text x="{mid}" y="150" text-anchor="middle" fill="{MUTED}">+x → +y, +y → −x</text>'
    )
    out.append(
        f'<text x="{w / 2}" y="{h - 14}" text-anchor="middle" fill="{INK}">'
        "Each row moves with its axis. The step of the old y axis changes sign. "
        "No value is recalculated.</text>"
    )
    out.append("</svg>")

    folder = os.path.join(HERE, "..", "paper", "figures")
    os.makedirs(folder, exist_ok=True)
    path = os.path.join(folder, "crossing_state.svg")
    with open(path, "w") as f:
        f.write("\n".join(out) + "\n")
    print("wrote", os.path.relpath(path))


PAPER = os.path.join(HERE, "..", "paper")

# How the paper names each way of crossing.
PAPER_NAMES = {
    "engine": "Permute, counted steps (ours)",
    "perm-run": "Permute, running total",
    "recompute": "Recompute at the crossing point",
    "fishpye": "Recompute from the origin (fishpye)",
    "cell-matrix": "Matrix per cell (HyperRogue)",
    "floor-1e-3": r"Nudge and floor, $\varepsilon = 10^{-3}$",
    "floor-1e-2": r"Nudge and floor, $\varepsilon = 10^{-2}$",
}
SHOWN = (1, 10, 100, 1000)


def sci(x):
    """A number as LaTeX, to two significant figures."""
    mantissa, exponent = f"{x:.1e}".split("e")
    return f"${mantissa}\\times10^{{{int(exponent)}}}$"


def paper_inputs():
    """Everything the paper reads: plot data and table bodies.

    The paper's numbers come from the confirmation run, whose seeds are not
    the ones the experiment was designed on.
    """
    rows = {}
    with open(os.path.join(RESULTS, "crossing_error_confirm.csv")) as f:
        for row in csv.DictReader(f):
            rows.setdefault(row["method"], {})[int(row["crossings"])] = row

    data = os.path.join(PAPER, "figures", "data")
    tables = os.path.join(PAPER, "tables")
    os.makedirs(data, exist_ok=True)
    os.makedirs(tables, exist_ok=True)

    for method in ("engine", "perm-run", "recompute", "fishpye", "cell-matrix"):
        with open(os.path.join(data, f"{method}.dat"), "w") as f:
            f.write("crossings median\n")
            for crossings in sorted(rows[method]):
                if crossings >= 1:
                    f.write(f"{crossings} {float(rows[method][crossings]['err_median']):.6e}\n")

    def off(row):
        return int(row["beside"]) + int(row["wrong"]) + int(row["lost"])

    with open(os.path.join(tables, "agreement.tex"), "w") as f:
        for method in ("engine", "perm-run", "recompute", "fishpye", "cell-matrix", "floor-1e-3"):
            cells = [f"{int(rows[method][c]['as_uncut']):,}".replace(",", "{,}") for c in SHOWN]
            f.write(f"{PAPER_NAMES[method]} & " + " & ".join(cells) + " \\\\\n")

    with open(os.path.join(tables, "accuracy.tex"), "w") as f:
        for method in ("engine", "perm-run", "recompute", "fishpye", "cell-matrix", "floor-1e-3", "floor-1e-2"):
            cells = []
            for c in SHOWN:
                row = rows[method][c]
                count = f"{off(row):,}".replace(",", "{,}")
                cells.append(f"{count} & {sci(float(row['err_median']))}")
            f.write(f"{PAPER_NAMES[method]} & " + " & ".join(cells) + " \\\\\n")

    with open(os.path.join(tables, "distance.tex"), "w") as f:
        cells = [
            "\\multicolumn{2}{c}{%s}"
            % f"{float(rows['engine'][c]['mean_distance']):,.0f}".replace(",", "{,}")
            for c in SHOWN
        ]
        f.write("Mean distance (voxels) & " + " & ".join(cells) + " \\\\\n")

    with open(os.path.join(RESULTS, "step_cost.csv")) as f:
        cost = list(csv.DictReader(f))
    fits = {r["method"]: r for r in cost if r["part"] == "fit"}
    with open(os.path.join(tables, "cost.tex"), "w") as out:
        for method in ("engine", "perm-run", "recompute", "fishpye", "floor-1e-2"):
            # A fit row holds the plain step in one column and the extra
            # cost of a crossing in the next.
            plain, extra = float(fits[method]["crossings_per_step"]), float(fits[method]["ns_per_step"])
            out.write(f"{PAPER_NAMES[method]} & {plain:.1f} & {extra:.0f} \\\\\n")

    with open(os.path.join(tables, "linktest.tex"), "w") as out:
        for name, label in LINK_LOOPS:
            best, median = link_times(cost, name)
            out.write(f"{label} & {best:.2f} & {median:.2f} \\\\\n")

    with open(os.path.join(RESULTS, "frames_confirm.csv")) as f, open(
        os.path.join(tables, "pixels.tex"), "w"
    ) as out:
        pixels = {r["method"]: r for r in csv.DictReader(line for line in f if not line.startswith("#"))}
        for method in ("engine", "perm-run", "recompute", "fishpye", "cell-matrix", "floor-1e-3", "floor-1e-2"):
            row = pixels[method]
            cells = [f"{int(row[k]):,}".replace(",", "{,}") for k in ("differ", "other_surface", "worst_view")]
            out.write(f"{PAPER_NAMES[method]} & " + " & ".join(cells) + " \\\\\n")
    print("wrote", os.path.relpath(data), "and", os.path.relpath(tables))


# The loops of step_cost's second part: name in the CSV, name in a table.
LINK_LOOPS = (
    ("loop without link test", "The cursor's loop, copied, without the link test"),
    ("loop with link test", "The same loop with the link test"),
    ("engine cursor", "The engine's cursor"),
)


def link_times(cost, name):
    """Fastest and median time per step of one loop of step_cost's part 2."""
    best = next(float(r["ns_per_step"]) for r in cost if r["part"] == "plain" and r["method"] == name)
    median = next(
        float(r["ns_per_step"]) for r in cost if r["part"] == "plain-median" and r["method"] == name
    )
    return best, median


def block(text, name, lines):
    """Replace what is between a table's two marker comments in the README."""
    start = f"<!-- {name}: written by figures.py -->"
    end = f"<!-- end {name} -->"
    a, b = text.index(start) + len(start), text.index(end)
    return text[:a] + "\n" + "\n".join(lines) + "\n" + text[b:]


def readme_tables():
    """The tables of experiments/README.md, from the run on the design seeds."""
    rows = {}
    with open(os.path.join(RESULTS, "crossing_error.csv")) as f:
        for row in csv.DictReader(f):
            rows.setdefault(row["method"], {})[int(row["crossings"])] = row
    with open(os.path.join(RESULTS, "step_cost.csv")) as f:
        cost = list(csv.DictReader(f))
    with open(os.path.join(RESULTS, "frames.csv")) as f:
        pixels = {r["method"]: r for r in csv.DictReader(line for line in f if not line.startswith("#"))}

    def off(row):
        return int(row["beside"]) + int(row["wrong"]) + int(row["lost"])

    def short(x):
        mantissa, exponent = f"{x:.1e}".split("e")
        return f"{mantissa}e{int(exponent)}"

    bar = "|---|---|---|---|---|"
    agreement = ["| Seams crossed | " + " | ".join(f"{c:,}" for c in SHOWN) + " |", bar]
    for method in ("engine", "perm-count", "perm-run", "recompute", "fishpye", "cell-matrix", "floor-1e-3"):
        cells = [f"{int(rows[method][c]['as_uncut']):,}" for c in SHOWN]
        agreement.append(f"| `{method}` | " + " | ".join(cells) + " |")

    heads = [f"{c:,} ({float(rows['engine'][c]['mean_distance']):,.0f})" for c in SHOWN]
    accuracy = ["| Seams crossed (mean distance) | " + " | ".join(heads) + " |", bar]
    for method in ("engine", "perm-run", "recompute", "fishpye", "cell-matrix", "floor-1e-3", "floor-1e-2"):
        accuracy.append(
            f"| `{method}`: off | " + " | ".join(f"{off(rows[method][c]):,}" for c in SHOWN) + " |"
        )
        if not method.startswith("floor"):
            errors = [short(float(rows[method][c]["err_median"])) for c in SHOWN]
            accuracy.append(f"| `{method}`: error | " + " | ".join(errors) + " |")

    fits = {r["method"]: r for r in cost if r["part"] == "fit"}
    costs = ["| | Plain step (ns) | Extra per crossing (ns) |", "|---|---|---|"]
    for method in ("engine", "perm-count", "perm-run", "recompute", "fishpye", "floor-1e-2"):
        plain, extra = float(fits[method]["crossings_per_step"]), float(fits[method]["ns_per_step"])
        costs.append(f"| `{method}` | {plain:.1f} | {extra:.0f} |")

    links = ["| | Fastest (ns a step) | Median (ns a step) |", "|---|---|---|"]
    for name, label in LINK_LOOPS:
        best, median = link_times(cost, name)
        links.append(f"| {label} | {best:.2f} | {median:.2f} |")

    shown = ["| | Pixels that differ | On another surface | Most in one view |", "|---|---|---|---|"]
    for method in ("engine", "perm-run", "recompute", "fishpye", "cell-matrix", "floor-1e-3", "floor-1e-2"):
        row = pixels[method]
        cells = [f"{int(row[k]):,}" for k in ("differ", "other_surface", "worst_view")]
        shown.append(f"| `{method}` | " + " | ".join(cells) + " |")

    path = os.path.join(HERE, "README.md")
    with open(path) as f:
        text = f.read()
    for name, lines in (
        ("agreement", agreement),
        ("accuracy", accuracy),
        ("cost", costs),
        ("link test", links),
        ("pixels", shown),
    ):
        text = block(text, name, lines)
    with open(path, "w") as f:
        f.write(text)
    print("wrote the tables of", os.path.relpath(path))


if __name__ == "__main__":
    crossing_error()
    crossing_state()
    paper_inputs()
    readme_tables()
