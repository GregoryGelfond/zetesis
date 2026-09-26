### Wall time (ms)

| Case | zetesis A1 | zetesis B1 | zetesis B2 | zetesis A2 | clingo A1 | clingo B1 | clingo B2 | clingo A2 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| scalability/n-queens | 11.682 | 16.445 | 17.120 | 11.083 | 6.552 | 5.198 | 6.528 | 6.552 |
| scalability/n-queens 8→9 | 14.697 | 24.085 | 24.164 | 15.292 | 9.033 | 9.078 | 9.078 | 9.060 |
| scalability/n-queens 8→10 | 25.701 | 41.185 | 41.151 | 25.927 | 28.527 | 27.740 | 28.505 | 28.495 |
| scalability/pigeonhole 7→5 | 7.869 | 8.412 | 8.395 | 8.304 | 5.233 | 5.192 | 5.196 | 5.195 |
| scalability/pigeonhole 7→6 | 8.257 | 8.972 | 8.335 | 8.397 | 5.842 | 5.810 | 5.803 | 5.812 |
| scalability/pigeonhole | 12.093 | 12.769 | 13.277 | 12.222 | 16.482 | 16.503 | 16.488 | 16.394 |
| n-queens/variant-02 | 50.779 | 56.487 | 56.228 | 51.523 | 118.175 | 117.795 | 118.723 | 118.181 |
| send-money/send-money | 14.093 | 15.945 | 15.845 | 13.567 | 11.489 | 12.158 | 12.778 | 12.167 |
| variant-04/05-larger-mix | 32.554 | 61.126 | 60.701 | 33.371 | 186.905 | 176.704 | 185.884 | 175.455 |
| einstein-riddle | Refused (R1) | 20.388 | 20.414 | Refused (R1) | 36.066 | 36.063 | 35.389 | 35.508 |

### Peak RSS (MiB)

| Case | zetesis A1 | zetesis B1 | zetesis B2 | zetesis A2 | clingo A1 | clingo B1 | clingo B2 | clingo A2 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| scalability/n-queens | 14.625 | 15.469 | 15.484 | 14.547 | 5.406 | 5.406 | 5.406 | 5.406 |
| scalability/n-queens 8→9 | 15.547 | 16.328 | 16.438 | 15.563 | 5.453 | 5.453 | 5.453 | 5.453 |
| scalability/n-queens 8→10 | 16.281 | 17.391 | 17.734 | 16.641 | 5.672 | 5.672 | 5.859 | 5.672 |
| scalability/pigeonhole 7→5 | 13.078 | 14.203 | 14.156 | 13.094 | 5.344 | 5.344 | 5.344 | 5.344 |
| scalability/pigeonhole 7→6 | 13.344 | 14.359 | 14.250 | 13.266 | 5.375 | 5.375 | 5.375 | 5.375 |
| scalability/pigeonhole | 13.906 | 14.844 | 14.953 | 13.969 | 5.609 | 5.609 | 5.609 | 5.609 |
| n-queens/variant-02 | 21.109 | 22.203 | 20.828 | 21.328 | 8.391 | 8.359 | 7.953 | 8.578 |
| send-money/send-money | 18.078 | 18.969 | 18.578 | 17.547 | 8.266 | 8.266 | 8.266 | 8.266 |
| variant-04/05-larger-mix | 22.141 | 22.734 | 22.953 | 22.125 | 19.422 | 19.391 | 19.391 | 19.391 |
| einstein-riddle | Refused (R1) | 20.969 | 20.719 | Refused (R1) | 19.453 | 19.453 | 19.453 | 19.453 |

R1: the old executable refuses Einstein during formula admission: Work limit 10,000,000; required at least 10,000,001 (source bytes 6155..6341). Subsequent timed and RSS positions are unattempted. No old successful duration or RSS is available.
