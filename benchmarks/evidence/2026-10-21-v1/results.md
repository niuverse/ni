# Benchmark results

Synthetic small-repository tasks and a separate four-task Polyglot subset. Development smoke runs are not benchmark scores.
API alias does not freeze the underlying model snapshot. No broad ranking is inferred.
Campaign cost upper bound: CNY 14.219348; requests without final usage: 0.

## Frozen formal comparisons

| Suite | Track | Harness | Resolved / completed / planned | Median seconds | Requests | Input tokens | Output tokens | CNY upper bound |
|---|---|---|---:|---:|---:|---:|---:|---:|
| polyglot | basic | niucode | 4 / 4 / 4 | 7.5 | 20 | 68894 | 4140 | 0.170908 |
| polyglot | basic | mini | 4 / 4 / 4 | 21.0 | 26 | 165438 | 13888 | 0.441980 |
| polyglot | basic | dsh | 4 / 4 / 4 | 16.7 | 28 | 204254 | 8350 | 0.475308 |
| polyglot | basic | pi | 4 / 4 / 4 | 8.2 | 23 | 117143 | 11653 | 0.327510 |
| polyglot | enhanced | niucode | 3 / 4 / 4 | 19.5 | 29 | 142016 | 15568 | 0.408576 |
| polyglot | enhanced | mini | 4 / 4 / 4 | 28.7 | 29 | 218713 | 13792 | 0.547762 |
| polyglot | enhanced | dsh | 4 / 4 / 4 | 19.0 | 31 | 290240 | 12972 | 0.684256 |
| polyglot | enhanced | pi | 4 / 4 / 4 | 14.0 | 29 | 166731 | 8339 | 0.400174 |
| synthetic | basic | niucode | 8 / 8 / 8 | 20.0 | 64 | 335852 | 36999 | 0.967696 |
| synthetic | basic | mini | 8 / 8 / 8 | 22.6 | 64 | 347968 | 39536 | 1.012224 |
| synthetic | basic | dsh | 8 / 8 / 8 | 21.7 | 66 | 465386 | 37462 | 1.230468 |
| synthetic | basic | pi | 7 / 8 / 8 | 12.1 | 45 | 157076 | 23948 | 0.505736 |
| synthetic | enhanced | niucode | 8 / 8 / 8 | 25.8 | 102 | 708552 | 52056 | 1.833552 |
| synthetic | enhanced | mini | 8 / 8 / 8 | 30.6 | 65 | 315414 | 40421 | 0.954196 |
| synthetic | enhanced | dsh | 8 / 8 / 8 | 29.4 | 88 | 831513 | 48869 | 2.053978 |
| synthetic | enhanced | pi | 8 / 8 / 8 | 18.8 | 72 | 388255 | 31263 | 1.026614 |

## All attempts, including development failures

| Run | Harness | Track | Status | Independent tests | Seconds | CNY upper bound |
|---|---|---|---|---|---:|---:|
| smoke-0a45aeeb-smoke-dsh-basic-0 | dsh | basic | passed | pass | 11.4 | 0.074086 |
| smoke-0a45aeeb-smoke-dsh-enhanced-0 | dsh | enhanced | passed | pass | 18.3 | 0.112960 |
| smoke-0a45aeeb-smoke-mini-basic-0 | mini | basic | harness_error | fail | 0.7 | 0.000000 |
| smoke-0a45aeeb-smoke-mini-enhanced-0 | mini | enhanced | harness_error | fail | 0.8 | 0.000000 |
| smoke-0a45aeeb-smoke-niucode-basic-0 | niucode | basic | passed | pass | 7.0 | 0.023798 |
| smoke-0a45aeeb-smoke-niucode-enhanced-0 | niucode | enhanced | passed | pass | 9.8 | 0.038928 |
| smoke-0a45aeeb-smoke-pi-basic-0 | pi | basic | passed | pass | 5.8 | 0.023890 |
| smoke-0a45aeeb-smoke-pi-enhanced-0 | pi | enhanced | passed | pass | 10.3 | 0.053348 |
| smoke-1159f058-smoke-dsh-basic-0 | dsh | basic | passed | pass | 11.6 | 0.059146 |
| smoke-1159f058-smoke-dsh-enhanced-0 | dsh | enhanced | passed | pass | 24.7 | 0.158494 |
| smoke-1159f058-smoke-mini-basic-0 | mini | basic | passed | pass | 16.1 | 0.044284 |
| smoke-1159f058-smoke-mini-enhanced-0 | mini | enhanced | passed | pass | 15.8 | 0.048160 |
| smoke-1159f058-smoke-niucode-basic-0 | niucode | basic | passed | pass | 7.8 | 0.029080 |
| smoke-1159f058-smoke-niucode-enhanced-0 | niucode | enhanced | passed | pass | 11.5 | 0.044850 |
| smoke-1159f058-smoke-pi-basic-0 | pi | basic | passed | pass | 5.6 | 0.024510 |
| smoke-1159f058-smoke-pi-enhanced-0 | pi | enhanced | passed | pass | 9.0 | 0.042980 |
| smoke-51e435c6-smoke-mini-basic-0 | mini | basic | harness_error | fail | 252.7 | 0.000000 |
| smoke-51e435c6-smoke-mini-enhanced-0 | mini | enhanced | harness_error | fail | 0.1 | 0.000000 |
| smoke-55635e30-smoke-dsh-basic-0 | dsh | basic | harness_error | fail | 0.6 | 0.000000 |
| smoke-55635e30-smoke-dsh-enhanced-0 | dsh | enhanced | harness_error | fail | 0.6 | 0.000000 |
| smoke-55635e30-smoke-mini-basic-0 | mini | basic | harness_error | fail | 0.6 | 0.000000 |
| smoke-55635e30-smoke-mini-enhanced-0 | mini | enhanced | harness_error | fail | 0.6 | 0.000000 |
| smoke-55635e30-smoke-niucode-basic-0 | niucode | basic | harness_error | fail | 0.6 | 0.000000 |
| smoke-55635e30-smoke-niucode-enhanced-0 | niucode | enhanced | harness_error | fail | 0.5 | 0.000000 |
| smoke-55635e30-smoke-pi-basic-0 | pi | basic | harness_error | fail | 0.6 | 0.000000 |
| smoke-55635e30-smoke-pi-enhanced-0 | pi | enhanced | harness_error | fail | 0.5 | 0.000000 |
| smoke-80f9ae0e-smoke-dsh-basic-0 | dsh | basic | passed | pass | 12.5 | 0.060284 |
| smoke-80f9ae0e-smoke-dsh-enhanced-0 | dsh | enhanced | passed | pass | 13.8 | 0.092172 |
| smoke-80f9ae0e-smoke-mini-basic-0 | mini | basic | passed | pass | 12.6 | 0.034198 |
| smoke-80f9ae0e-smoke-mini-enhanced-0 | mini | enhanced | passed | pass | 18.9 | 0.057636 |
| smoke-80f9ae0e-smoke-niucode-basic-0 | niucode | basic | passed | pass | 8.3 | 0.029970 |
| smoke-80f9ae0e-smoke-niucode-enhanced-0 | niucode | enhanced | passed | pass | 12.2 | 0.049940 |
| smoke-80f9ae0e-smoke-pi-basic-0 | pi | basic | passed | pass | 6.2 | 0.025274 |
| smoke-80f9ae0e-smoke-pi-enhanced-0 | pi | enhanced | passed | pass | 9.9 | 0.050422 |
| test-polyglot-py-grade-school-dsh-basic-0 | dsh | basic | passed | pass | 11.8 | 0.082644 |
| test-polyglot-py-grade-school-dsh-enhanced-0 | dsh | enhanced | passed | pass | 16.1 | 0.133218 |
| test-polyglot-py-grade-school-mini-basic-0 | mini | basic | passed | pass | 12.5 | 0.049156 |
| test-polyglot-py-grade-school-mini-enhanced-0 | mini | enhanced | passed | pass | 14.3 | 0.070564 |
| test-polyglot-py-grade-school-niucode-basic-0 | niucode | basic | passed | pass | 7.2 | 0.040876 |
| test-polyglot-py-grade-school-niucode-enhanced-0 | niucode | enhanced | passed | pass | 11.3 | 0.080362 |
| test-polyglot-py-grade-school-pi-basic-0 | pi | basic | passed | pass | 5.9 | 0.045670 |
| test-polyglot-py-grade-school-pi-enhanced-0 | pi | enhanced | passed | pass | 9.1 | 0.078080 |
| test-polyglot-py-transpose-dsh-basic-0 | dsh | basic | passed | pass | 32.5 | 0.156462 |
| test-polyglot-py-transpose-dsh-enhanced-0 | dsh | enhanced | passed | pass | 41.9 | 0.218574 |
| test-polyglot-py-transpose-mini-basic-0 | mini | basic | passed | pass | 51.2 | 0.212054 |
| test-polyglot-py-transpose-mini-enhanced-0 | mini | enhanced | passed | pass | 34.5 | 0.128936 |
| test-polyglot-py-transpose-niucode-basic-0 | niucode | basic | passed | pass | 11.0 | 0.037958 |
| test-polyglot-py-transpose-niucode-enhanced-0 | niucode | enhanced | failed | fail | 42.8 | 0.079752 |
| test-polyglot-py-transpose-pi-basic-0 | pi | basic | passed | pass | 58.4 | 0.177206 |
| test-polyglot-py-transpose-pi-enhanced-0 | pi | enhanced | passed | pass | 31.9 | 0.147298 |
| test-polyglot-rs-variable-length-quantity-dsh-basic-0 | dsh | basic | passed | pass | 19.5 | 0.154006 |
| test-polyglot-rs-variable-length-quantity-dsh-enhanced-0 | dsh | enhanced | passed | pass | 21.2 | 0.201246 |
| test-polyglot-rs-variable-length-quantity-mini-basic-0 | mini | basic | passed | pass | 23.1 | 0.103960 |
| test-polyglot-rs-variable-length-quantity-mini-enhanced-0 | mini | enhanced | passed | pass | 32.3 | 0.211702 |
| test-polyglot-rs-variable-length-quantity-niucode-basic-0 | niucode | basic | passed | pass | 7.8 | 0.048294 |
| test-polyglot-rs-variable-length-quantity-niucode-enhanced-0 | niucode | enhanced | passed | pass | 20.9 | 0.134854 |
| test-polyglot-rs-variable-length-quantity-pi-basic-0 | pi | basic | passed | pass | 8.9 | 0.058554 |
| test-polyglot-rs-variable-length-quantity-pi-enhanced-0 | pi | enhanced | passed | pass | 15.0 | 0.091094 |
| test-polyglot-rs-word-count-dsh-basic-0 | dsh | basic | passed | pass | 14.0 | 0.082196 |
| test-polyglot-rs-word-count-dsh-enhanced-0 | dsh | enhanced | passed | pass | 16.9 | 0.131218 |
| test-polyglot-rs-word-count-mini-basic-0 | mini | basic | passed | pass | 18.9 | 0.076810 |
| test-polyglot-rs-word-count-mini-enhanced-0 | mini | enhanced | passed | pass | 25.0 | 0.136560 |
| test-polyglot-rs-word-count-niucode-basic-0 | niucode | basic | passed | pass | 6.9 | 0.043780 |
| test-polyglot-rs-word-count-niucode-enhanced-0 | niucode | enhanced | passed | pass | 18.1 | 0.113608 |
| test-polyglot-rs-word-count-pi-basic-0 | pi | basic | passed | pass | 7.6 | 0.046080 |
| test-polyglot-rs-word-count-pi-enhanced-0 | pi | enhanced | passed | pass | 13.0 | 0.083702 |
| test-py-config-dsh-basic-0 | dsh | basic | passed | pass | 18.6 | 0.080920 |
| test-py-config-dsh-enhanced-0 | dsh | enhanced | passed | pass | 20.3 | 0.106582 |
| test-py-config-mini-basic-0 | mini | basic | passed | pass | 21.6 | 0.064370 |
| test-py-config-mini-enhanced-0 | mini | enhanced | passed | pass | 23.6 | 0.084830 |
| test-py-config-niucode-basic-0 | niucode | basic | passed | pass | 25.2 | 0.113764 |
| test-py-config-niucode-enhanced-0 | niucode | enhanced | passed | pass | 24.4 | 0.115412 |
| test-py-config-pi-basic-0 | pi | basic | passed | pass | 13.3 | 0.059684 |
| test-py-config-pi-enhanced-0 | pi | enhanced | passed | pass | 13.8 | 0.071212 |
| test-py-csv-dsh-basic-0 | dsh | basic | passed | pass | 70.1 | 0.348602 |
| test-py-csv-dsh-enhanced-0 | dsh | enhanced | passed | pass | 89.8 | 0.535314 |
| test-py-csv-mini-basic-0 | mini | basic | passed | pass | 88.4 | 0.410772 |
| test-py-csv-mini-enhanced-0 | mini | enhanced | passed | pass | 71.2 | 0.248100 |
| test-py-csv-niucode-basic-0 | niucode | basic | passed | pass | 42.9 | 0.240942 |
| test-py-csv-niucode-enhanced-0 | niucode | enhanced | passed | pass | 88.8 | 0.639958 |
| test-py-csv-pi-basic-0 | pi | basic | failed | fail | 44.6 | 0.079450 |
| test-py-csv-pi-enhanced-0 | pi | enhanced | passed | pass | 60.7 | 0.345280 |
| test-py-pagination-dsh-basic-0 | dsh | basic | passed | pass | 18.1 | 0.078344 |
| test-py-pagination-dsh-enhanced-0 | dsh | enhanced | passed | pass | 22.3 | 0.132638 |
| test-py-pagination-mini-basic-0 | mini | basic | passed | pass | 22.0 | 0.061128 |
| test-py-pagination-mini-enhanced-0 | mini | enhanced | passed | pass | 30.5 | 0.094204 |
| test-py-pagination-niucode-basic-0 | niucode | basic | passed | pass | 21.1 | 0.098312 |
| test-py-pagination-niucode-enhanced-0 | niucode | enhanced | passed | pass | 45.2 | 0.354836 |
| test-py-pagination-pi-basic-0 | pi | basic | passed | pass | 10.8 | 0.032318 |
| test-py-pagination-pi-enhanced-0 | pi | enhanced | passed | pass | 16.7 | 0.060690 |
| test-py-tasks-dsh-basic-0 | dsh | basic | passed | pass | 20.2 | 0.084788 |
| test-py-tasks-dsh-enhanced-0 | dsh | enhanced | passed | pass | 19.8 | 0.124408 |
| test-py-tasks-mini-basic-0 | mini | basic | passed | pass | 23.2 | 0.069064 |
| test-py-tasks-mini-enhanced-0 | mini | enhanced | passed | pass | 21.9 | 0.074766 |
| test-py-tasks-niucode-basic-0 | niucode | basic | passed | pass | 14.5 | 0.057772 |
| test-py-tasks-niucode-enhanced-0 | niucode | enhanced | passed | pass | 23.8 | 0.121438 |
| test-py-tasks-pi-basic-0 | pi | basic | passed | pass | 9.4 | 0.040238 |
| test-py-tasks-pi-enhanced-0 | pi | enhanced | passed | pass | 16.6 | 0.070472 |
| test-rs-cache-dsh-basic-0 | dsh | basic | passed | pass | 23.2 | 0.136928 |
| test-rs-cache-dsh-enhanced-0 | dsh | enhanced | passed | pass | 44.4 | 0.351782 |
| test-rs-cache-mini-basic-0 | mini | basic | passed | pass | 21.1 | 0.071230 |
| test-rs-cache-mini-enhanced-0 | mini | enhanced | passed | pass | 23.4 | 0.085138 |
| test-rs-cache-niucode-basic-0 | niucode | basic | passed | pass | 14.9 | 0.062188 |
| test-rs-cache-niucode-enhanced-0 | niucode | enhanced | passed | pass | 19.9 | 0.097270 |
| test-rs-cache-pi-basic-0 | pi | basic | passed | pass | 8.9 | 0.033126 |
| test-rs-cache-pi-enhanced-0 | pi | enhanced | passed | pass | 20.8 | 0.127764 |
| test-rs-dry-run-dsh-basic-0 | dsh | basic | passed | pass | 38.8 | 0.184014 |
| test-rs-dry-run-dsh-enhanced-0 | dsh | enhanced | passed | pass | 36.6 | 0.228720 |
| test-rs-dry-run-mini-basic-0 | mini | basic | passed | pass | 44.7 | 0.176992 |
| test-rs-dry-run-mini-enhanced-0 | mini | enhanced | passed | pass | 36.0 | 0.145878 |
| test-rs-dry-run-niucode-basic-0 | niucode | basic | passed | pass | 18.8 | 0.062844 |
| test-rs-dry-run-niucode-enhanced-0 | niucode | enhanced | passed | pass | 27.1 | 0.155554 |
| test-rs-dry-run-pi-basic-0 | pi | basic | passed | pass | 25.9 | 0.097266 |
| test-rs-dry-run-pi-enhanced-0 | pi | enhanced | passed | pass | 26.9 | 0.145154 |
| test-rs-scan-dsh-basic-0 | dsh | basic | passed | pass | 38.0 | 0.195496 |
| test-rs-scan-dsh-enhanced-0 | dsh | enhanced | passed | pass | 63.2 | 0.443316 |
| test-rs-scan-mini-basic-0 | mini | basic | passed | pass | 30.1 | 0.101734 |
| test-rs-scan-mini-enhanced-0 | mini | enhanced | passed | pass | 32.0 | 0.109198 |
| test-rs-scan-niucode-basic-0 | niucode | basic | passed | pass | 52.1 | 0.284062 |
| test-rs-scan-niucode-enhanced-0 | niucode | enhanced | passed | pass | 36.1 | 0.232770 |
| test-rs-scan-pi-basic-0 | pi | basic | passed | pass | 30.6 | 0.139560 |
| test-rs-scan-pi-enhanced-0 | pi | enhanced | passed | pass | 25.9 | 0.129148 |
| test-rs-unicode-dsh-basic-0 | dsh | basic | passed | pass | 18.2 | 0.121376 |
| test-rs-unicode-dsh-enhanced-0 | dsh | enhanced | passed | pass | 20.3 | 0.131218 |
| test-rs-unicode-mini-basic-0 | mini | basic | passed | pass | 17.9 | 0.056934 |
| test-rs-unicode-mini-enhanced-0 | mini | enhanced | passed | pass | 30.7 | 0.112082 |
| test-rs-unicode-niucode-basic-0 | niucode | basic | passed | pass | 14.0 | 0.047812 |
| test-rs-unicode-niucode-enhanced-0 | niucode | enhanced | passed | pass | 21.9 | 0.116314 |
| test-rs-unicode-pi-basic-0 | pi | basic | passed | pass | 7.0 | 0.024094 |
| test-rs-unicode-pi-enhanced-0 | pi | enhanced | passed | pass | 15.0 | 0.076894 |

Plans (denominators):
- frozen-plan-polyglot.json: 32 planned (scored).
- frozen-plan-synthetic.json: 64 planned (scored).
- smoke-plan-0a45aeeb.json: 8 planned (development).
- smoke-plan-1159f058.json: 8 planned (development).
- smoke-plan-51e435c6.json: 2 planned (development).
- smoke-plan-55635e30.json: 8 planned (development).
- smoke-plan-80f9ae0e.json: 8 planned (development).
