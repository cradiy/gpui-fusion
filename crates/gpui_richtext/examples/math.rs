#[path = "support/graphics_preview.rs"]
mod preview;

fn main() {
    preview::run(
        r#"# Mathematical expressions

$$
\int_{-\infty}^{\infty} e^{-t^2}\,dt = \sqrt{\pi}
$$

$$
\begin{pmatrix} p & q \\ r & s \end{pmatrix}^{-1}
= \frac{1}{ps-qr}\begin{pmatrix} s & -q \\ -r & p \end{pmatrix}
$$

$$
\sum_{k=0}^{m} \binom{m}{k} z^k = (1+z)^m
$$
"#,
    );
}
