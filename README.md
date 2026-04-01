# descend.tex

> [!NOTE]
> I welcome any Pull Requests, as this is still pre-mature and not stable project.

> LaTeX is just a flat-earther. We taught it a new axis.

This is not a new language, but a new way to write the LaTeX you are used to.

Inspired by Python's philosophy.
Indents means block structure and every inline contents are just turned into raw latex.

---

## What we do

Spread block structures into a single LaTeX string.

## What we don't

Parse or validate inline LaTeX. That's your compiler's job.

![Basic flow](https://github.com/user-attachments/assets/64160714-4705-4e3f-a229-acaf07871e97)

---

## Why

LaTeX is powerful, but it only thinks horizontally. Concepts that are
two-dimensional — fractions, matrices, underbraces — get flattened
into a single line.

`descend.tex` enable you to write them as you think.

---

## Syntax

### The rule

- Indentation = structure
- Block commands: no `\`, children go below
- Inline: raw LaTeX, just like you know it

### Inline commands

Inline commands (starting with `\`) are treated as plain text —
`descend.tex` is not responsible for them. Your LaTeX compiler is.

### Examples

**Fraction**
```latex
% LaTeX
\frac{-b + \sqrt{b^2 - 4ac}}{2a}
```
```dtex
# descend.tex
frac
  \sqrt{b^2 - 4ac} + -b
  2a
```

**Matrix**
```latex
% LaTeX
\begin{pmatrix} a & b \\ c & d \end{pmatrix}
```
```dtex
# descend.tex
pmatrix
  a b
  c d
```

**Underbrace**
```latex
% LaTeX
\underbrace{a + b + c}_{S}
```
```dtex
# descend.tex
underbrace
  a + b + c
  S
```

---

## Installation
```sh
cargo install --git https://github.com/mtugb/descend.tex
```

---

## Usage
```sh
dtex input.dtex
dtex input.dtex --replacements replacements.toml
```

---

## Replacements

Optional inline substitution via a TOML file.
```toml
[replacements]
alpha = "\\alpha"
inf   = "\\infty"
```

Pass it explicitly with `--replacements`. No default replacements exist.

---

## About the name

The word "Decends" inspirates you go down vertically.
And, this project let you make structures by going down lines.
After all, laTeX only went sideways, we taught it to go down.
