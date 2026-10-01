# corpus: zetesis 14 threads, clingo one thread

Baseline `7c717c05` and candidate `159c4563`. Wall times are milliseconds: median [minimum, maximum] over seven timed fresh processes. RSS is the median peak in MiB from three separate memory runs. Both builds use two warmups. Cases/profiles are zero-based. The companion evidence JSON supplies complete hashes, sample arrays, phases and qualification fingerprints. Suites overlap.

| Case/profile and complete entry | Parameters | Baseline zetesis | Candidate zetesis | Baseline clingo | Candidate clingo | Zetesis RSS B → C | Clingo RSS B → C |
|---|---|---:|---:|---:|---:|---:|---:|
| 0/0: `scenarios/equality-generalized-tsp/01-basic.lp` | default source | 7.788 [7.754, 7.867] | 7.834 [7.745, 7.876] | 4.675 [4.664, 4.721] | 4.692 [4.661, 4.701] | 14.859 → 14.719 | 5.453 → 5.453 |
| 1/0: `scenarios/equality-generalized-tsp/02-larger.lp` | default source | 7.762 [7.723, 7.875] | 7.752 [7.607, 7.824] | 4.678 [4.632, 4.701] | 4.672 [4.640, 4.685] | 15.062 → 15.078 | 5.453 → 5.453 |
| 2/0: `scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp` | default source | 6.247 [6.213, 6.306] | 6.247 [6.217, 6.353] | 4.683 [4.655, 4.700] | 4.676 [4.662, 4.699] | 14.391 → 14.328 | 5.250 → 5.250 |
| 3/0: `scenarios/shortest-path/variant-01/01-basic.lp` | default source | 6.250 [6.194, 6.269] | 6.245 [6.202, 6.283] | 4.671 [4.651, 4.705] | 4.686 [4.637, 4.709] | 14.469 → 14.469 | 5.438 → 5.438 |
| 4/0: `scenarios/shortest-path/variant-01/02-start-equals-end.lp` | default source | 6.233 [6.203, 6.281] | 6.250 [6.199, 6.265] | 4.676 [4.650, 4.703] | 4.673 [4.655, 4.704] | 14.438 → 14.406 | 5.375 → 5.375 |
| 5/0: `scenarios/shortest-path/variant-01/03-zero-cost-detour.lp` | default source | 6.247 [6.207, 6.273] | 6.212 [6.190, 6.309] | 4.689 [4.158, 4.698] | 4.682 [4.650, 4.702] | 14.438 → 14.422 | 5.391 → 5.391 |
| 6/0: `scenarios/shortest-path/variant-01/04-no-path.lp` | default source | 6.248 [6.222, 6.258] | 6.254 [6.206, 6.287] | 4.686 [4.644, 4.719] | 4.677 [4.651, 4.711] | 14.391 → 14.312 | 5.266 → 5.266 |
| 7/0: `scenarios/shortest-path/variant-01/05-multi-path.lp` | default source | 6.228 [6.191, 6.245] | 6.244 [6.187, 7.792] | 4.694 [4.631, 4.701] | 4.677 [4.641, 4.701] | 14.641 → 14.609 | 5.484 → 5.484 |
| 8/0: `scenarios/shortest-path/variant-01/06-layered-dag.lp` | default source | 9.256 [9.189, 9.325] | 9.285 [9.258, 9.317] | 6.162 [4.643, 6.174] | 6.161 [4.652, 6.196] | 16.938 → 16.750 | 5.641 → 5.641 |
| 9/0: `scenarios/shortest-path/variant-01/07-cycles.lp` | default source | 7.759 [7.457, 7.840] | 7.790 [7.737, 7.826] | 4.665 [4.649, 4.692] | 4.661 [4.649, 4.697] | 15.078 → 15.031 | 5.500 → 5.500 |
| 10/0: `scenarios/shortest-path/variant-01/08-negative-weights.lp` | default source | 6.252 [6.191, 7.846] | 6.271 [6.204, 7.783] | 4.669 [4.635, 4.679] | 4.662 [4.642, 4.690] | 14.781 → 14.750 | 5.406 → 5.406 |
| 11/0: `scenarios/shortest-path/variant-02/01-basic.lp` | default source | 6.210 [6.188, 6.282] | 6.239 [6.199, 7.819] | 4.670 [4.643, 4.709] | 4.685 [4.640, 4.701] | 14.688 → 14.688 | 5.453 → 5.453 |
| 12/0: `scenarios/shortest-path/variant-02/02-start-equals-end.lp` | default source | 6.258 [6.180, 7.816] | 6.269 [6.194, 7.795] | 4.662 [4.656, 4.703] | 4.667 [4.645, 4.693] | 14.641 → 14.688 | 5.406 → 5.406 |
| 13/0: `scenarios/shortest-path/variant-02/03-before-forces-detour.lp` | default source | 7.728 [6.183, 7.785] | 6.232 [6.192, 7.802] | 4.658 [4.641, 4.701] | 4.668 [4.649, 4.694] | 14.844 → 14.781 | 5.469 → 5.469 |
| 14/0: `scenarios/shortest-path/variant-02/04-after-forces-extension.lp` | default source | 6.252 [6.187, 7.783] | 6.261 [6.193, 7.808] | 4.686 [4.645, 4.728] | 4.685 [4.664, 4.696] | 14.734 → 14.703 | 5.484 → 5.484 |
| 15/0: `scenarios/shortest-path/variant-02/05-ordering-unsat.lp` | default source | 6.231 [6.183, 6.263] | 6.223 [6.196, 6.271] | 4.666 [4.643, 4.681] | 4.682 [4.660, 4.700] | 14.547 → 14.578 | 5.391 → 5.391 |
| 16/0: `scenarios/shortest-path/variant-02/06-layered-dag-before.lp` | default source | 9.276 [9.224, 9.298] | 9.266 [9.230, 9.330] | 6.177 [6.154, 6.200] | 6.186 [4.690, 6.212] | 16.688 → 16.406 | 5.688 → 5.688 |
| 17/0: `scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp` | default source | 9.266 [9.241, 9.313] | 9.279 [9.242, 9.365] | 6.178 [6.149, 6.200] | 6.183 [4.662, 6.197] | 16.406 → 16.359 | 5.688 → 5.688 |
| 18/0: `scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp` | default source | 7.775 [6.192, 7.804] | 7.784 [6.231, 7.811] | 4.670 [4.647, 4.679] | 4.683 [4.660, 4.689] | 14.859 → 14.828 | 5.469 → 5.469 |
| 19/0: `scenarios/shortest-path/variant-02/09-negative-weights.lp` | default source | 7.753 [6.211, 7.791] | 7.750 [6.221, 7.800] | 4.670 [4.650, 4.692] | 4.659 [4.644, 4.664] | 15.000 → 14.984 | 5.422 → 5.422 |
| 20/0: `scenarios/shortest-path/variant-03/01-basic.lp` | default source | 6.222 [6.195, 6.279] | 6.233 [6.197, 7.789] | 4.687 [4.654, 4.706] | 4.668 [4.648, 4.682] | 14.797 → 14.688 | 5.453 → 5.453 |
| 21/0: `scenarios/shortest-path/variant-03/02-start-equals-end.lp` | default source | 6.213 [6.182, 6.265] | 6.234 [6.200, 7.796] | 4.679 [4.657, 4.695] | 4.676 [4.646, 4.690] | 14.719 → 14.703 | 5.391 → 5.391 |
| 22/0: `scenarios/shortest-path/variant-03/03-budget-forces-detour.lp` | default source | 6.225 [6.193, 7.767] | 6.238 [6.188, 7.801] | 4.669 [4.649, 4.694] | 4.659 [4.638, 4.699] | 14.703 → 14.641 | 5.438 → 5.438 |
| 23/0: `scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp` | default source | 6.216 [6.190, 6.264] | 6.222 [6.193, 6.268] | 4.673 [4.641, 4.702] | 4.678 [4.663, 4.699] | 14.656 → 14.734 | 5.391 → 5.391 |
| 24/0: `scenarios/shortest-path/variant-03/05-budget-unsat.lp` | default source | 7.756 [7.705, 7.791] | 7.786 [7.723, 7.845] | 4.673 [4.656, 4.687] | 4.695 [4.655, 4.731] | 15.000 → 14.859 | 5.516 → 5.516 |
| 25/0: `scenarios/shortest-path/variant-03/06-layered-dag-cap.lp` | default source | 10.778 [10.749, 10.870] | 10.786 [10.594, 10.844] | 6.184 [6.143, 7.200] | 6.172 [6.137, 6.357] | 18.406 → 17.938 | 5.938 → 5.938 |
| 26/0: `scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp` | default source | 10.794 [10.768, 10.836] | 10.821 [10.769, 10.860] | 6.165 [6.138, 6.185] | 6.164 [6.145, 7.267] | 17.844 → 18.344 | 5.938 → 5.938 |
| 27/0: `scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp` | default source | 7.767 [6.199, 7.814] | 7.767 [7.722, 7.833] | 4.650 [4.641, 4.656] | 4.662 [4.639, 4.700] | 14.875 → 14.891 | 5.484 → 5.484 |
| 28/0: `scenarios/shortest-path/variant-03/09-negative-weights.lp` | default source | 7.766 [6.203, 7.806] | 7.786 [7.731, 7.809] | 4.663 [4.657, 4.688] | 4.663 [4.651, 4.698] | 15.094 → 14.984 | 5.422 → 5.422 |
| 29/0: `scenarios/shortest-path/variant-04/01-basic.lp` | default source | 7.752 [6.197, 7.797] | 6.213 [6.192, 7.782] | 4.678 [4.636, 4.695] | 4.652 [4.626, 4.691] | 14.891 → 14.922 | 5.500 → 5.500 |
| 30/0: `scenarios/shortest-path/variant-04/02-start-equals-end.lp` | default source | 7.740 [6.188, 7.813] | 7.763 [6.243, 7.795] | 4.664 [4.632, 4.692] | 4.675 [4.637, 4.688] | 14.766 → 14.688 | 5.453 → 5.453 |
| 31/0: `scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp` | default source | 7.742 [7.716, 7.769] | 7.752 [6.195, 7.796] | 4.659 [4.636, 4.687] | 4.658 [4.638, 4.683] | 14.938 → 14.922 | 5.500 → 5.500 |
| 32/0: `scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp` | default source | 7.765 [6.189, 7.786] | 7.794 [6.230, 7.824] | 4.647 [4.640, 4.673] | 4.661 [4.645, 4.671] | 14.859 → 14.844 | 5.484 → 5.484 |
| 33/0: `scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp` | default source | 7.765 [6.201, 7.796] | 7.742 [7.723, 7.811] | 4.666 [4.642, 4.691] | 4.654 [4.640, 4.684] | 15.109 → 15.031 | 5.469 → 5.469 |
| 34/0: `scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp` | default source | 10.777 [10.729, 10.808] | 10.802 [10.730, 10.807] | 6.161 [6.137, 6.211] | 6.152 [6.131, 6.182] | 17.469 → 17.562 | 6.000 → 6.000 |
| 35/0: `scenarios/shortest-path/variant-04/07-layered-dag-combined.lp` | default source | 10.762 [10.297, 10.844] | 10.784 [10.618, 10.853] | 6.152 [6.126, 6.175] | 6.155 [6.134, 6.184] | 17.562 → 17.688 | 6.000 → 6.000 |
| 36/0: `scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp` | default source | 7.785 [7.771, 7.837] | 7.773 [7.622, 7.819] | 4.643 [4.633, 4.672] | 4.646 [4.637, 4.659] | 15.234 → 15.156 | 5.531 → 5.531 |
| 37/0: `scenarios/shortest-path/variant-04/09-negative-weights.lp` | default source | 7.735 [7.715, 7.835] | 7.782 [7.722, 7.834] | 4.648 [4.637, 4.669] | 4.660 [4.641, 4.682] | 15.188 → 15.078 | 5.469 → 5.469 |
| 38/0: `scenarios/task-allocation/variant-01/01-basic.lp` | default source | 6.253 [6.206, 6.292] | 6.287 [6.235, 7.786] | 4.683 [4.647, 4.707] | 4.690 [4.655, 4.732] | 14.062 → 14.047 | 5.297 → 5.297 |
| 39/0: `scenarios/task-allocation/variant-01/02-agent-reuse.lp` | default source | 6.247 [6.094, 6.258] | 6.245 [6.204, 6.272] | 4.680 [4.651, 4.686] | 4.692 [4.647, 4.714] | 14.266 → 14.328 | 5.359 → 5.359 |
| 40/0: `scenarios/task-allocation/variant-01/03-selective-compatibility.lp` | default source | 6.261 [6.215, 6.277] | 6.231 [6.135, 7.793] | 4.669 [4.649, 4.683] | 4.669 [4.639, 4.685] | 14.141 → 14.078 | 5.312 → 5.312 |
| 41/0: `scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp` | default source | 6.237 [6.213, 6.311] | 6.281 [6.214, 6.299] | 4.673 [4.660, 4.702] | 4.681 [4.241, 4.689] | 13.891 → 13.953 | 5.188 → 5.188 |
| 42/0: `scenarios/task-allocation/variant-01/05-larger-mix.lp` | default source | 7.748 [6.208, 7.754] | 7.748 [7.716, 7.779] | 4.677 [4.653, 4.703] | 4.658 [4.636, 4.709] | 14.891 → 14.797 | 5.359 → 5.359 |
| 43/0: `scenarios/task-allocation/variant-02/01-basic.lp` | default source | 6.212 [6.197, 7.808] | 7.758 [6.095, 7.786] | 4.675 [4.653, 4.685] | 4.673 [4.656, 4.682] | 14.547 → 14.469 | 5.453 → 5.453 |
| 44/0: `scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp` | default source | 7.734 [6.198, 7.820] | 7.746 [6.205, 7.807] | 4.673 [4.648, 4.684] | 4.681 [4.645, 4.690] | 14.750 → 14.688 | 5.453 → 5.453 |
| 45/0: `scenarios/task-allocation/variant-02/03-cost-dominates.lp` | default source | 7.735 [7.256, 7.809] | 7.766 [6.202, 7.796] | 4.685 [4.657, 4.701] | 4.669 [4.652, 4.689] | 14.734 → 14.625 | 5.453 → 5.453 |
| 46/0: `scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp` | default source | 6.245 [6.185, 6.291] | 7.746 [6.189, 7.818] | 4.672 [4.650, 4.682] | 4.675 [4.646, 4.698] | 14.391 → 14.453 | 5.281 → 5.281 |
| 47/0: `scenarios/task-allocation/variant-02/05-larger-mix.lp` | default source | 9.258 [8.880, 9.333] | 9.277 [8.825, 9.306] | 4.677 [4.642, 6.146] | 4.659 [4.649, 4.692] | 16.359 → 16.406 | 5.562 → 5.562 |
| 48/0: `scenarios/task-allocation/variant-03/01-basic.lp` | default source | 6.265 [6.211, 6.276] | 6.238 [6.200, 7.831] | 4.654 [4.647, 4.694] | 4.666 [4.643, 4.697] | 14.266 → 14.266 | 5.391 → 5.391 |
| 49/0: `scenarios/task-allocation/variant-03/02-multiple-groups.lp` | default source | 6.237 [6.210, 6.248] | 6.249 [6.205, 7.793] | 4.664 [4.490, 4.693] | 4.680 [4.661, 4.691] | 14.562 → 14.547 | 5.391 → 5.391 |
| 50/0: `scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp` | default source | 6.252 [6.197, 7.803] | 6.241 [6.191, 7.830] | 4.667 [4.643, 4.693] | 4.672 [4.658, 4.694] | 14.406 → 14.344 | 5.391 → 5.391 |
| 51/0: `scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp` | default source | 6.260 [6.237, 6.293] | 6.265 [6.233, 6.299] | 4.684 [4.666, 4.690] | 4.666 [4.567, 4.691] | 14.156 → 14.109 | 5.297 → 5.297 |
| 52/0: `scenarios/task-allocation/variant-03/05-larger-mix.lp` | default source | 6.225 [6.219, 7.801] | 6.222 [6.204, 7.779] | 4.696 [4.659, 4.805] | 4.669 [4.652, 4.699] | 14.875 → 14.719 | 5.391 → 5.391 |
| 53/0: `scenarios/task-allocation/variant-04/01-basic.lp` | default source | 10.749 [9.764, 10.902] | 9.592 [9.224, 11.063] | 22.744 [22.731, 24.288] | 22.743 [22.710, 22.766] | 16.969 → 16.922 | 8.797 → 8.797 |
| 54/0: `scenarios/task-allocation/variant-04/02-precedence.lp` | default source | 9.278 [7.712, 9.357] | 7.792 [7.721, 18.401] | 6.171 [6.165, 6.181] | 6.172 [6.141, 6.185] | 15.766 → 15.562 | 5.828 → 5.828 |
| 55/0: `scenarios/task-allocation/variant-04/03-agent-serialization.lp` | default source | 9.265 [9.228, 10.883] | 9.265 [9.251, 9.327] | 24.245 [24.229, 25.764] | 24.269 [24.210, 25.756] | 15.984 → 15.812 | 8.000 → 8.000 |
| 56/0: `scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp` | default source | 7.795 [7.737, 7.891] | 7.748 [7.714, 7.822] | 4.663 [4.640, 4.684] | 4.649 [4.625, 4.665] | 15.250 → 15.094 | 5.547 → 5.547 |
| 57/0: `scenarios/task-allocation/variant-04/05-larger-mix.lp` | default source | 37.997 [37.386, 39.745] | 36.785 [34.678, 37.155] | 188.384 [176.351, 193.090] | 185.558 [164.503, 195.947] | 25.312 → 25.141 | 19.406 → 19.453 |
| 58/0: `scenarios/traveling-salesman/variant-01/01-basic.lp` | default source | 7.915 [7.843, 8.061] | 7.974 [7.871, 8.055] | 4.714 [4.650, 4.740] | 4.715 [4.662, 4.750] | 14.562 → 14.484 | 5.484 → 5.484 |
| 59/0: `scenarios/traveling-salesman/variant-01/02-multiple-tours.lp` | default source | 7.764 [7.707, 7.818] | 7.790 [7.735, 7.851] | 4.671 [4.649, 4.685] | 4.671 [4.637, 4.688] | 15.250 → 15.312 | 5.531 → 5.531 |
| 60/0: `scenarios/traveling-salesman/variant-01/03-asymmetric.lp` | default source | 7.774 [7.448, 7.810] | 7.792 [7.729, 9.253] | 4.647 [4.636, 4.677] | 4.652 [4.634, 4.665] | 15.438 → 15.453 | 5.609 → 5.609 |
| 61/0: `scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp` | default source | 6.251 [6.197, 6.276] | 6.276 [6.205, 8.001] | 4.653 [4.643, 4.698] | 4.659 [4.644, 4.686] | 14.234 → 14.156 | 5.266 → 5.266 |
| 62/0: `scenarios/traveling-salesman/variant-01/05-ring.lp` | default source | 7.811 [7.608, 7.954] | 7.994 [7.728, 8.828] | 4.684 [4.655, 6.171] | 4.695 [4.679, 6.202] | 16.547 → 16.609 | 5.609 → 5.609 |
| 63/0: `scenarios/traveling-salesman/variant-02/01-basic.lp` | default source | 7.754 [7.703, 7.853] | 7.773 [7.729, 7.865] | 4.667 [4.649, 4.676] | 4.685 [4.644, 4.692] | 14.828 → 14.750 | 5.500 → 5.500 |
| 64/0: `scenarios/traveling-salesman/variant-02/02-single-salesman.lp` | default source | 6.255 [6.192, 7.791] | 7.720 [6.198, 7.821] | 4.663 [4.648, 4.697] | 4.672 [4.661, 4.696] | 14.766 → 14.672 | 5.438 → 5.438 |
| 65/0: `scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp` | default source | 6.251 [6.192, 7.824] | 7.543 [6.204, 7.829] | 4.666 [4.653, 4.698] | 4.686 [4.653, 4.752] | 14.656 → 14.500 | 5.406 → 5.406 |
| 66/0: `scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp` | default source | 7.728 [6.222, 7.780] | 7.788 [7.730, 7.913] | 4.687 [4.652, 4.697] | 4.666 [4.651, 4.734] | 14.812 → 14.719 | 5.500 → 5.500 |
| 67/0: `scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp` | default source | 7.734 [7.706, 7.831] | 7.770 [7.332, 7.852] | 4.667 [4.624, 4.697] | 4.653 [4.642, 4.690] | 14.828 → 14.812 | 5.484 → 5.484 |
| 68/0: `scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp` | default source | 7.781 [7.729, 7.817] | 7.796 [7.756, 7.883] | 4.656 [4.174, 4.717] | 4.664 [4.298, 4.722] | 15.047 → 14.906 | 5.484 → 5.484 |
| 69/0: `scenarios/traveling-salesman/variant-03/01-basic.lp` | default source | 6.276 [6.195, 7.753] | 6.260 [6.186, 7.795] | 4.666 [4.638, 4.674] | 4.662 [4.641, 4.695] | 14.703 → 14.625 | 5.453 → 5.453 |
| 70/0: `scenarios/traveling-salesman/variant-03/02-single-salesman.lp` | default source | 7.739 [6.184, 7.784] | 6.279 [6.193, 7.819] | 4.665 [4.165, 4.689] | 4.668 [4.643, 4.695] | 14.734 → 14.609 | 5.453 → 5.453 |
| 71/0: `scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp` | default source | 6.208 [6.183, 6.256] | 6.250 [6.202, 7.771] | 4.650 [4.224, 4.668] | 4.668 [4.660, 4.699] | 14.609 → 14.516 | 5.406 → 5.406 |
| 72/0: `scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp` | default source | 7.765 [7.722, 7.790] | 7.744 [7.735, 7.803] | 4.695 [4.647, 4.712] | 4.676 [4.639, 4.687] | 15.000 → 14.844 | 5.516 → 5.516 |
| 73/0: `scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp` | default source | 7.724 [7.710, 7.780] | 7.782 [7.625, 7.962] | 4.678 [4.651, 4.688] | 4.664 [4.643, 4.688] | 15.047 → 15.109 | 5.484 → 5.484 |
| 74/0: `scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp` | default source | 7.753 [7.366, 7.795] | 7.743 [7.358, 7.807] | 4.659 [4.635, 4.714] | 4.668 [4.641, 4.694] | 14.938 → 14.906 | 5.438 → 5.438 |
| 75/0: `scenarios/traveling-salesman/variant-04/01-basic.lp` | default source | 7.740 [7.701, 7.791] | 7.770 [7.704, 7.825] | 4.660 [4.641, 4.684] | 4.662 [4.641, 4.682] | 15.359 → 15.250 | 5.562 → 5.562 |
| 76/0: `scenarios/traveling-salesman/variant-04/02-single-salesman.lp` | default source | 7.781 [7.311, 7.812] | 7.769 [7.719, 7.811] | 4.645 [4.632, 4.659] | 4.654 [4.628, 4.683] | 15.281 → 15.156 | 5.578 → 5.578 |
| 77/0: `scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp` | default source | 7.748 [7.723, 7.947] | 7.762 [7.713, 7.830] | 4.656 [4.639, 4.673] | 4.652 [4.639, 4.681] | 15.031 → 15.016 | 5.531 → 5.531 |
| 78/0: `scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp` | default source | 7.746 [7.429, 7.799] | 7.773 [7.348, 7.848] | 4.650 [4.529, 4.677] | 4.659 [4.646, 4.691] | 14.938 → 14.953 | 5.516 → 5.516 |
| 79/0: `scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp` | default source | 10.770 [10.730, 10.910] | 9.264 [9.228, 10.797] | 4.649 [4.638, 6.205] | 4.661 [4.638, 4.675] | 15.734 → 15.641 | 5.578 → 5.578 |
| 80/0: `scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp` | default source | 9.275 [9.107, 10.861] | 9.141 [8.849, 9.363] | 4.661 [4.623, 5.730] | 4.636 [4.626, 4.689] | 15.781 → 15.562 | 5.578 → 5.578 |
| 81/0: `scenarios/traveling-salesman/variant-05/01-basic.lp` | default source | 7.735 [7.707, 7.774] | 7.761 [7.715, 7.810] | 4.658 [4.638, 4.670] | 4.656 [4.638, 4.697] | 15.438 → 15.359 | 5.578 → 5.578 |
| 82/0: `scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp` | default source | 7.750 [7.662, 7.790] | 7.783 [7.750, 7.929] | 4.648 [4.639, 4.661] | 4.649 [4.631, 4.673] | 15.219 → 15.141 | 5.562 → 5.562 |
| 83/0: `scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp` | default source | 7.782 [7.723, 8.986] | 7.779 [7.722, 7.831] | 4.651 [4.639, 6.279] | 4.650 [4.637, 4.655] | 15.062 → 15.094 | 5.531 → 5.531 |
| 84/0: `scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp` | default source | 7.777 [7.711, 7.822] | 7.768 [7.737, 7.790] | 4.639 [4.628, 4.728] | 4.654 [4.638, 4.734] | 15.219 → 15.219 | 5.578 → 5.578 |
| 85/0: `scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp` | default source | 10.769 [10.726, 10.833] | 10.752 [9.229, 10.811] | 4.647 [4.623, 6.182] | 4.639 [4.623, 4.674] | 15.781 → 15.734 | 5.594 → 5.594 |
| 86/0: `scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp` | default source | 9.255 [9.224, 9.466] | 9.268 [9.247, 9.388] | 4.665 [4.619, 5.968] | 4.644 [4.619, 4.693] | 15.812 → 15.766 | 5.578 → 5.578 |
| 87/0: `standalone/n-queens/variant-01.lp` | n=8 | 15.282 [15.257, 15.331] | 12.389 [12.267, 12.757] | 6.185 [6.145, 6.218] | 6.180 [4.634, 6.211] | 15.891 → 15.375 | 5.422 → 5.422 |
| 88/0: `standalone/n-queens/variant-02.lp` | n=8 | 33.907 [32.745, 35.040] | 34.199 [32.652, 36.637] | 122.167 [120.538, 123.765] | 122.107 [120.542, 122.156] | 32.797 → 37.469 | 8.203 → 8.484 |
| 89/0: `standalone/n-queens/variant-03.lp` | n=8 | 13.883 [13.856, 13.965] | 13.908 [13.879, 13.927] | 6.194 [6.171, 6.361] | 6.199 [6.173, 6.217] | 16.297 → 16.547 | 5.438 → 5.438 |
| 90/0: `standalone/n-queens/variant-04.lp` | n=8 | 9.274 [9.248, 9.316] | 9.321 [9.300, 9.353] | 6.181 [6.165, 6.186] | 6.181 [6.158, 6.184] | 15.828 → 15.828 | 5.469 → 5.469 |
| 91/0: `standalone/n-queens/variant-05.lp` | n=8 | 9.273 [9.191, 9.317] | 9.296 [9.283, 9.323] | 6.191 [6.166, 6.209] | 6.189 [6.166, 6.217] | 16.500 → 16.609 | 5.531 → 5.531 |
| 92/0: `standalone/n-queens/variant-06.lp` | n=8 | 9.288 [9.260, 9.310] | 9.295 [9.270, 9.336] | 6.172 [6.164, 6.231] | 6.186 [6.171, 6.225] | 16.266 → 16.250 | 5.562 → 5.562 |
| 93/0: `standalone/send-money/send-money.lp` | default source | 15.296 [13.771, 15.369] | 15.342 [15.311, 15.424] | 12.200 [12.173, 12.221] | 12.198 [12.190, 12.228] | 19.875 → 21.094 | 8.281 → 8.281 |

## Grounding, solving and driver intervals

Median milliseconds, baseline → candidate. Native grounding/solving use the recorded typed stages; driver uses the maintained comparison. Lazy grounding is interleaved with solving, so its separate grounding stage is unavailable. Native driver excludes source loading and statistics output. Clingo non-solving is its reported Total−Solve, including grounding and preprocessing; it is not independently measured pure grounding. These scopes differ from process wall time. Fine source phases retained in the evidence JSON are nested intervals and must not be added to these totals.

| Case/profile and complete entry | Parameters | Native grounding B → C | Native solving B → C | Native driver B → C | Clingo non-solving B → C | Clingo solving B → C |
|---|---|---:|---:|---:|---:|---:|
| 0/0: `scenarios/equality-generalized-tsp/01-basic.lp` | default source | 0.651 → 0.605 | 0.371 → 0.364 | 1.817 → 1.782 | 1.000 → 1.000 | 0.000 → 0.000 |
| 1/0: `scenarios/equality-generalized-tsp/02-larger.lp` | default source | 1.096 → 1.029 | 0.441 → 0.416 | 2.351 → 2.321 | 1.000 → 1.000 | 0.000 → 0.000 |
| 2/0: `scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp` | default source | 0.323 → 0.309 | 0.227 → 0.221 | 1.194 → 1.197 | 0.000 → 1.000 | 0.000 → 0.000 |
| 3/0: `scenarios/shortest-path/variant-01/01-basic.lp` | default source | 0.325 → 0.318 | 0.268 → 0.236 | 1.309 → 1.298 | 1.000 → 1.000 | 0.000 → 0.000 |
| 4/0: `scenarios/shortest-path/variant-01/02-start-equals-end.lp` | default source | 0.263 → 0.258 | 0.243 → 0.221 | 1.187 → 1.191 | 1.000 → 1.000 | 0.000 → 0.000 |
| 5/0: `scenarios/shortest-path/variant-01/03-zero-cost-detour.lp` | default source | 0.303 → 0.301 | 0.263 → 0.229 | 1.292 → 1.267 | 1.000 → 1.000 | 0.000 → 0.000 |
| 6/0: `scenarios/shortest-path/variant-01/04-no-path.lp` | default source | 0.288 → 0.284 | 0.212 → 0.193 | 1.184 → 1.183 | 1.000 → 1.000 | 0.000 → 0.000 |
| 7/0: `scenarios/shortest-path/variant-01/05-multi-path.lp` | default source | 0.534 → 0.504 | 0.337 → 0.304 | 1.622 → 1.561 | 1.000 → 1.000 | 0.000 → 0.000 |
| 8/0: `scenarios/shortest-path/variant-01/06-layered-dag.lp` | default source | 1.607 → 1.531 | 0.994 → 1.002 | 3.704 → 3.653 | 2.000 → 2.000 | 0.000 → 0.000 |
| 9/0: `scenarios/shortest-path/variant-01/07-cycles.lp` | default source | 0.688 → 0.660 | 0.435 → 0.442 | 1.984 → 1.971 | 1.000 → 1.000 | 0.000 → 0.000 |
| 10/0: `scenarios/shortest-path/variant-01/08-negative-weights.lp` | default source | 0.434 → 0.431 | 0.380 → 0.373 | 1.565 → 1.555 | 1.000 → 1.000 | 0.000 → 0.000 |
| 11/0: `scenarios/shortest-path/variant-02/01-basic.lp` | default source | 0.344 → 0.343 | 0.274 → 0.256 | 1.450 → 1.412 | 1.000 → 1.000 | 0.000 → 0.000 |
| 12/0: `scenarios/shortest-path/variant-02/02-start-equals-end.lp` | default source | 0.305 → 0.300 | 0.221 → 0.321 | 1.358 → 1.426 | 1.000 → 1.000 | 0.000 → 0.000 |
| 13/0: `scenarios/shortest-path/variant-02/03-before-forces-detour.lp` | default source | 0.418 → 0.402 | 0.391 → 0.273 | 1.653 → 1.517 | 1.000 → 1.000 | 0.000 → 0.000 |
| 14/0: `scenarios/shortest-path/variant-02/04-after-forces-extension.lp` | default source | 0.408 → 0.396 | 0.260 → 0.239 | 1.491 → 1.491 | 1.000 → 1.000 | 0.000 → 0.000 |
| 15/0: `scenarios/shortest-path/variant-02/05-ordering-unsat.lp` | default source | 0.328 → 0.322 | 0.255 → 0.201 | 1.380 → 1.360 | 1.000 → 1.000 | 0.000 → 0.000 |
| 16/0: `scenarios/shortest-path/variant-02/06-layered-dag-before.lp` | default source | 1.760 → 1.667 | 0.836 → 0.969 | 3.775 → 3.833 | 2.000 → 2.000 | 0.000 → 0.000 |
| 17/0: `scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp` | default source | 1.763 → 1.679 | 0.800 → 0.846 | 3.795 → 3.698 | 2.000 → 2.000 | 0.000 → 0.000 |
| 18/0: `scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp` | default source | 0.438 → 0.420 | 0.398 → 0.380 | 1.679 → 1.671 | 1.000 → 1.000 | 0.000 → 0.000 |
| 19/0: `scenarios/shortest-path/variant-02/09-negative-weights.lp` | default source | 0.478 → 0.458 | 0.394 → 0.376 | 1.712 → 1.675 | 1.000 → 1.000 | 0.000 → 0.000 |
| 20/0: `scenarios/shortest-path/variant-03/01-basic.lp` | default source | 0.373 → 0.365 | 0.300 → 0.340 | 1.501 → 1.528 | 1.000 → 1.000 | 0.000 → 0.000 |
| 21/0: `scenarios/shortest-path/variant-03/02-start-equals-end.lp` | default source | 0.324 → 0.320 | 0.246 → 0.190 | 1.372 → 1.336 | 1.000 → 1.000 | 0.000 → 0.000 |
| 22/0: `scenarios/shortest-path/variant-03/03-budget-forces-detour.lp` | default source | 0.402 → 0.393 | 0.247 → 0.227 | 1.479 → 1.441 | 1.000 → 1.000 | 0.000 → 0.000 |
| 23/0: `scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp` | default source | 0.359 → 0.347 | 0.229 → 0.221 | 1.391 → 1.387 | 1.000 → 1.000 | 0.000 → 0.000 |
| 24/0: `scenarios/shortest-path/variant-03/05-budget-unsat.lp` | default source | 0.666 → 0.621 | 0.307 → 0.287 | 1.831 → 1.766 | 1.000 → 1.000 | 0.000 → 0.000 |
| 25/0: `scenarios/shortest-path/variant-03/06-layered-dag-cap.lp` | default source | 2.548 → 2.418 | 1.357 → 1.351 | 5.127 → 4.970 | 2.000 → 2.000 | 0.000 → 0.000 |
| 26/0: `scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp` | default source | 2.547 → 2.383 | 1.358 → 1.329 | 5.143 → 5.030 | 3.000 → 2.000 | 0.000 → 0.000 |
| 27/0: `scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp` | default source | 0.502 → 0.502 | 0.395 → 0.382 | 1.764 → 1.719 | 1.000 → 1.000 | 0.000 → 0.000 |
| 28/0: `scenarios/shortest-path/variant-03/09-negative-weights.lp` | default source | 0.540 → 0.531 | 0.391 → 0.395 | 1.787 → 1.785 | 1.000 → 1.000 | 0.000 → 0.000 |
| 29/0: `scenarios/shortest-path/variant-04/01-basic.lp` | default source | 0.406 → 0.389 | 0.361 → 0.251 | 1.661 → 1.600 | 1.000 → 1.000 | 0.000 → 0.000 |
| 30/0: `scenarios/shortest-path/variant-04/02-start-equals-end.lp` | default source | 0.370 → 0.368 | 0.278 → 0.336 | 1.555 → 1.623 | 1.000 → 1.000 | 0.000 → 0.000 |
| 31/0: `scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp` | default source | 0.525 → 0.508 | 0.296 → 0.367 | 1.756 → 1.809 | 1.000 → 1.000 | 0.000 → 0.000 |
| 32/0: `scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp` | default source | 0.524 → 0.526 | 0.242 → 0.231 | 1.721 → 1.710 | 1.000 → 1.000 | 0.000 → 0.000 |
| 33/0: `scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp` | default source | 0.582 → 0.563 | 0.409 → 0.380 | 1.964 → 1.923 | 1.000 → 1.000 | 0.000 → 0.000 |
| 34/0: `scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp` | default source | 2.749 → 2.641 | 1.424 → 1.049 | 5.541 → 5.037 | 2.000 → 2.000 | 0.000 → 0.000 |
| 35/0: `scenarios/shortest-path/variant-04/07-layered-dag-combined.lp` | default source | 2.769 → 2.644 | 1.072 → 1.266 | 5.257 → 5.303 | 2.000 → 2.000 | 0.000 → 0.000 |
| 36/0: `scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp` | default source | 0.811 → 0.763 | 0.460 → 0.419 | 2.289 → 2.251 | 1.000 → 1.000 | 0.000 → 0.000 |
| 37/0: `scenarios/shortest-path/variant-04/09-negative-weights.lp` | default source | 0.609 → 0.568 | 0.348 → 0.338 | 1.946 → 1.928 | 1.000 → 1.000 | 0.000 → 0.000 |
| 38/0: `scenarios/task-allocation/variant-01/01-basic.lp` | default source | 0.237 → 0.230 | 0.368 → 0.354 | 1.123 → 1.119 | 0.000 → 0.000 | 0.000 → 0.000 |
| 39/0: `scenarios/task-allocation/variant-01/02-agent-reuse.lp` | default source | 0.277 → 0.280 | 0.386 → 0.365 | 1.189 → 1.212 | 0.000 → 0.000 | 0.000 → 0.000 |
| 40/0: `scenarios/task-allocation/variant-01/03-selective-compatibility.lp` | default source | 0.271 → 0.267 | 0.304 → 0.362 | 1.108 → 1.163 | 0.000 → 0.000 | 0.000 → 0.000 |
| 41/0: `scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp` | default source | 0.217 → 0.213 | 0.222 → 0.196 | 0.914 → 0.910 | 0.000 → 0.000 | 0.000 → 0.000 |
| 42/0: `scenarios/task-allocation/variant-01/05-larger-mix.lp` | default source | 0.422 → 0.421 | 0.700 → 0.767 | 1.755 → 1.836 | 0.000 → 0.000 | 0.000 → 0.000 |
| 43/0: `scenarios/task-allocation/variant-02/01-basic.lp` | default source | 0.567 → 0.558 | 0.390 → 0.383 | 1.628 → 1.637 | 1.000 → 1.000 | 0.000 → 0.000 |
| 44/0: `scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp` | default source | 0.663 → 0.632 | 0.438 → 0.414 | 1.843 → 1.834 | 1.000 → 1.000 | 0.000 → 0.000 |
| 45/0: `scenarios/task-allocation/variant-02/03-cost-dominates.lp` | default source | 0.644 → 0.630 | 0.423 → 0.414 | 1.804 → 1.818 | 1.000 → 1.000 | 0.000 → 0.000 |
| 46/0: `scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp` | default source | 0.509 → 0.497 | 0.231 → 0.351 | 1.433 → 1.536 | 0.000 → 1.000 | 0.000 → 0.000 |
| 47/0: `scenarios/task-allocation/variant-02/05-larger-mix.lp` | default source | 1.441 → 1.409 | 1.199 → 1.151 | 3.653 → 3.517 | 1.000 → 1.000 | 0.000 → 0.000 |
| 48/0: `scenarios/task-allocation/variant-03/01-basic.lp` | default source | 0.266 → 0.271 | 0.268 → 0.348 | 1.159 → 1.260 | 0.000 → 0.000 | 0.000 → 0.000 |
| 49/0: `scenarios/task-allocation/variant-03/02-multiple-groups.lp` | default source | 0.380 → 0.371 | 0.347 → 0.381 | 1.430 → 1.487 | 1.000 → 0.000 | 0.000 → 0.000 |
| 50/0: `scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp` | default source | 0.313 → 0.302 | 0.294 → 0.374 | 1.273 → 1.359 | 0.000 → 0.000 | 0.000 → 0.000 |
| 51/0: `scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp` | default source | 0.216 → 0.213 | 0.245 → 0.206 | 1.049 → 1.024 | 0.000 → 0.000 | 0.000 → 0.000 |
| 52/0: `scenarios/task-allocation/variant-03/05-larger-mix.lp` | default source | 0.453 → 0.447 | 0.492 → 0.459 | 1.698 → 1.699 | 1.000 → 1.000 | 0.000 → 0.000 |
| 53/0: `scenarios/task-allocation/variant-04/01-basic.lp` | default source | 2.278 → 1.819 | 1.437 → 1.472 | 5.022 → 4.674 | 14.000 → 14.000 | 5.000 → 5.000 |
| 54/0: `scenarios/task-allocation/variant-04/02-precedence.lp` | default source | 1.229 → 1.064 | 0.535 → 0.510 | 2.971 → 2.838 | 2.000 → 2.000 | 0.000 → 0.000 |
| 55/0: `scenarios/task-allocation/variant-04/03-agent-serialization.lp` | default source | 2.126 → 1.673 | 0.660 → 0.633 | 4.008 → 3.522 | 20.000 → 21.000 | 0.000 → 0.000 |
| 56/0: `scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp` | default source | 0.542 → 0.518 | 0.239 → 0.274 | 1.884 → 1.881 | 1.000 → 1.000 | 0.000 → 0.000 |
| 57/0: `scenarios/task-allocation/variant-04/05-larger-mix.lp` | default source | 7.380 → 5.556 | 14.020 → 13.336 | 32.816 → 30.502 | 73.000 → 74.000 | 110.000 → 110.000 |
| 58/0: `scenarios/traveling-salesman/variant-01/01-basic.lp` | default source | 0.478 → 0.479 | 0.402 → 0.382 | 1.606 → 1.611 | 1.000 → 1.000 | 0.000 → 0.000 |
| 59/0: `scenarios/traveling-salesman/variant-01/02-multiple-tours.lp` | default source | 0.645 → 0.638 | 0.570 → 0.510 | 1.992 → 1.898 | 1.000 → 1.000 | 0.000 → 0.000 |
| 60/0: `scenarios/traveling-salesman/variant-01/03-asymmetric.lp` | default source | 0.814 → 0.774 | 0.562 → 0.570 | 2.133 → 2.107 | 1.000 → 1.000 | 0.000 → 0.000 |
| 61/0: `scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp` | default source | 0.360 → 0.340 | 0.234 → 0.211 | 1.205 → 1.181 | 0.000 → 0.000 | 0.000 → 0.000 |
| 62/0: `scenarios/traveling-salesman/variant-01/05-ring.lp` | default source | 0.973 → 0.962 | 1.056 → 1.040 | 2.847 → 2.822 | 1.000 → 1.000 | 0.000 → 0.000 |
| 63/0: `scenarios/traveling-salesman/variant-02/01-basic.lp` | default source | 0.637 → 0.603 | 0.369 → 0.354 | 1.790 → 1.788 | 1.000 → 1.000 | 0.000 → 0.000 |
| 64/0: `scenarios/traveling-salesman/variant-02/02-single-salesman.lp` | default source | 0.432 → 0.408 | 0.363 → 0.352 | 1.571 → 1.527 | 1.000 → 1.000 | 0.000 → 0.000 |
| 65/0: `scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp` | default source | 0.542 → 0.506 | 0.264 → 0.265 | 1.557 → 1.505 | 1.000 → 1.000 | 0.000 → 0.000 |
| 66/0: `scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp` | default source | 0.546 → 0.512 | 0.314 → 0.358 | 1.636 → 1.638 | 1.000 → 1.000 | 0.000 → 0.000 |
| 67/0: `scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp` | default source | 0.799 → 0.719 | 0.355 → 0.360 | 2.000 → 1.963 | 1.000 → 1.000 | 0.000 → 0.000 |
| 68/0: `scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp` | default source | 0.828 → 0.772 | 0.442 → 0.425 | 2.120 → 2.053 | 1.000 → 1.000 | 0.000 → 0.000 |
| 69/0: `scenarios/traveling-salesman/variant-03/01-basic.lp` | default source | 0.570 → 0.516 | 0.310 → 0.292 | 1.638 → 1.578 | 1.000 → 1.000 | 0.000 → 0.000 |
| 70/0: `scenarios/traveling-salesman/variant-03/02-single-salesman.lp` | default source | 0.434 → 0.390 | 0.368 → 0.340 | 1.569 → 1.499 | 1.000 → 1.000 | 0.000 → 0.000 |
| 71/0: `scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp` | default source | 0.552 → 0.504 | 0.229 → 0.239 | 1.509 → 1.491 | 1.000 → 1.000 | 0.000 → 0.000 |
| 72/0: `scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp` | default source | 0.716 → 0.677 | 0.428 → 0.409 | 1.969 → 1.908 | 1.000 → 1.000 | 0.000 → 0.000 |
| 73/0: `scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp` | default source | 1.443 → 1.272 | 0.495 → 0.515 | 2.805 → 2.688 | 1.000 → 1.000 | 0.000 → 0.000 |
| 74/0: `scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp` | default source | 0.894 → 0.814 | 0.425 → 0.427 | 2.132 → 2.081 | 1.000 → 1.000 | 0.000 → 0.000 |
| 75/0: `scenarios/traveling-salesman/variant-04/01-basic.lp` | default source | 1.159 → 1.066 | 0.431 → 0.333 | 2.736 → 2.611 | 1.000 → 1.000 | 0.000 → 0.000 |
| 76/0: `scenarios/traveling-salesman/variant-04/02-single-salesman.lp` | default source | 0.842 → 0.795 | 0.326 → 0.327 | 2.313 → 2.266 | 1.000 → 1.000 | 0.000 → 0.000 |
| 77/0: `scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp` | default source | 0.683 → 0.635 | 0.225 → 0.227 | 2.007 → 1.968 | 1.000 → 1.000 | 0.000 → 0.000 |
| 78/0: `scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp` | default source | 0.531 → 0.514 | 0.220 → 0.218 | 1.820 → 1.798 | 1.000 → 1.000 | 0.000 → 0.000 |
| 79/0: `scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp` | default source | 2.974 → 2.666 | 0.643 → 0.601 | 4.879 → 4.513 | 1.000 → 1.000 | 0.000 → 0.000 |
| 80/0: `scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp` | default source | 2.231 → 2.083 | 0.568 → 0.544 | 4.088 → 3.924 | 1.000 → 1.000 | 0.000 → 0.000 |
| 81/0: `scenarios/traveling-salesman/variant-05/01-basic.lp` | default source | 1.142 → 1.049 | 0.426 → 0.363 | 2.728 → 2.595 | 1.000 → 1.000 | 0.000 → 0.000 |
| 82/0: `scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp` | default source | 0.695 → 0.660 | 0.303 → 0.369 | 2.157 → 2.158 | 1.000 → 1.000 | 0.000 → 0.000 |
| 83/0: `scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp` | default source | 0.693 → 0.651 | 0.254 → 0.243 | 2.042 → 2.030 | 1.000 → 1.000 | 0.000 → 0.000 |
| 84/0: `scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp` | default source | 0.675 → 0.651 | 0.297 → 0.264 | 2.102 → 2.075 | 1.000 → 1.000 | 0.000 → 0.000 |
| 85/0: `scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp` | default source | 2.993 → 2.746 | 0.639 → 0.609 | 4.910 → 4.656 | 1.000 → 1.000 | 0.000 → 0.000 |
| 86/0: `scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp` | default source | 2.260 → 2.105 | 0.573 → 0.550 | 4.145 → 3.977 | 1.000 → 1.000 | 0.000 → 0.000 |
| 87/0: `standalone/n-queens/variant-01.lp` | n=8 | 7.940 → 5.993 | 0.735 → 0.817 | 9.631 → 7.624 | 1.000 → 1.000 | 1.000 → 1.000 |
| 88/0: `standalone/n-queens/variant-02.lp` | n=8 | 6.817 → 6.253 | 19.885 → 21.303 | 27.864 → 28.714 | 1.000 → 1.000 | 117.000 → 117.000 |
| 89/0: `standalone/n-queens/variant-03.lp` | n=8 | 5.970 → 5.683 | 0.747 → 0.753 | 7.738 → 7.428 | 1.000 → 1.000 | 1.000 → 1.000 |
| 90/0: `standalone/n-queens/variant-04.lp` | n=8 | 1.771 → 1.534 | 0.553 → 0.616 | 3.395 → 3.115 | 1.000 → 1.000 | 1.000 → 1.000 |
| 91/0: `standalone/n-queens/variant-05.lp` | n=8 | 1.555 → 1.538 | 0.759 → 0.740 | 4.135 → 3.981 | 1.000 → 1.000 | 1.000 → 1.000 |
| 92/0: `standalone/n-queens/variant-06.lp` | n=8 | 1.586 → 1.513 | 0.768 → 0.793 | 4.018 → 3.884 | 1.000 → 1.000 | 1.000 → 1.000 |
| 93/0: `standalone/send-money/send-money.lp` | default source | 5.109 → 5.144 | 2.534 → 2.503 | 8.992 → 9.000 | 7.000 → 7.000 | 1.000 → 1.000 |

All reported positions passed; all source inputs were unchanged. The full native qualification-family fingerprint matches before and after for every row. Clingo qualification compares displayed answers and costs; it cannot establish equality of hidden clingo interpretations. Its reported non-solving time includes preprocessing.
