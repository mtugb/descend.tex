#!/bin/bash
cargo run -p cli -- sample.htex          # .tex生成
latexmk -pdfdvi -latex=platex sample.tex
