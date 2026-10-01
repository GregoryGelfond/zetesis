# corpus: zetesis 4 threads, clingo one thread

Baseline `7c717c05` and candidate `159c4563`. Wall times are milliseconds: median [minimum, maximum] over seven timed fresh processes. RSS is the median peak in MiB from three separate memory runs. Both builds use two warmups. Cases/profiles are zero-based. The companion evidence JSON supplies complete hashes, sample arrays, phases and qualification fingerprints. Suites overlap.

| Case/profile and complete entry | Parameters | Baseline zetesis | Candidate zetesis | Baseline clingo | Candidate clingo | Zetesis RSS B → C | Clingo RSS B → C |
|---|---|---:|---:|---:|---:|---:|---:|
| 0/0: `scenarios/equality-generalized-tsp/01-basic.lp` | default source | 7.780 [6.221, 7.835] | 7.746 [6.217, 7.871] | 4.681 [4.654, 4.720] | 4.670 [4.654, 4.699] | 14.688 → 14.734 | 5.453 → 5.453 |
| 1/0: `scenarios/equality-generalized-tsp/02-larger.lp` | default source | 7.711 [7.295, 7.749] | 7.767 [7.732, 7.823] | 4.676 [4.640, 4.702] | 4.666 [4.655, 4.699] | 14.844 → 14.969 | 5.453 → 5.453 |
| 2/0: `scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp` | default source | 6.265 [6.217, 6.296] | 6.261 [6.217, 6.291] | 4.673 [4.656, 4.685] | 4.674 [4.644, 4.684] | 14.375 → 14.312 | 5.250 → 5.250 |
| 3/0: `scenarios/shortest-path/variant-01/01-basic.lp` | default source | 6.246 [6.207, 6.270] | 6.218 [6.191, 6.273] | 4.662 [4.648, 4.710] | 4.671 [4.649, 4.698] | 14.391 → 14.453 | 5.438 → 5.438 |
| 4/0: `scenarios/shortest-path/variant-01/02-start-equals-end.lp` | default source | 6.261 [6.210, 6.269] | 6.259 [6.218, 6.267] | 4.672 [4.656, 4.690] | 4.674 [4.657, 4.701] | 14.328 → 14.391 | 5.375 → 5.375 |
| 5/0: `scenarios/shortest-path/variant-01/03-zero-cost-detour.lp` | default source | 6.246 [6.227, 6.274] | 6.239 [6.211, 6.262] | 4.663 [4.654, 4.687] | 4.673 [4.649, 4.694] | 14.406 → 14.438 | 5.391 → 5.391 |
| 6/0: `scenarios/shortest-path/variant-01/04-no-path.lp` | default source | 6.265 [6.199, 6.288] | 6.253 [6.233, 6.282] | 4.681 [4.648, 4.725] | 4.679 [4.657, 4.691] | 14.281 → 14.375 | 5.266 → 5.266 |
| 7/0: `scenarios/shortest-path/variant-01/05-multi-path.lp` | default source | 6.209 [6.175, 6.260] | 6.236 [6.186, 7.779] | 4.685 [4.649, 4.694] | 4.652 [4.637, 4.709] | 14.609 → 14.641 | 5.484 → 5.484 |
| 8/0: `scenarios/shortest-path/variant-01/06-layered-dag.lp` | default source | 9.250 [9.237, 9.254] | 9.260 [9.220, 9.273] | 4.668 [4.661, 6.172] | 4.676 [4.646, 6.188] | 15.953 → 16.047 | 5.641 → 5.641 |
| 9/0: `scenarios/shortest-path/variant-01/07-cycles.lp` | default source | 7.732 [7.276, 7.809] | 7.743 [6.175, 7.811] | 4.666 [4.628, 4.676] | 4.680 [4.630, 4.686] | 14.922 → 14.906 | 5.500 → 5.500 |
| 10/0: `scenarios/shortest-path/variant-01/08-negative-weights.lp` | default source | 6.257 [6.181, 6.264] | 6.238 [6.204, 6.274] | 4.666 [4.644, 4.689] | 4.672 [4.664, 4.759] | 14.719 → 14.812 | 5.406 → 5.406 |
| 11/0: `scenarios/shortest-path/variant-02/01-basic.lp` | default source | 6.200 [6.192, 6.245] | 6.246 [6.226, 7.855] | 4.672 [4.636, 4.686] | 4.680 [4.646, 4.717] | 14.688 → 14.578 | 5.453 → 5.453 |
| 12/0: `scenarios/shortest-path/variant-02/02-start-equals-end.lp` | default source | 6.248 [6.232, 6.293] | 6.252 [6.215, 6.302] | 4.678 [4.661, 4.695] | 4.675 [4.645, 4.688] | 14.656 → 14.625 | 5.406 → 5.406 |
| 13/0: `scenarios/shortest-path/variant-02/03-before-forces-detour.lp` | default source | 6.241 [6.210, 6.287] | 6.225 [6.205, 6.264] | 4.663 [4.644, 4.701] | 4.684 [4.654, 4.707] | 14.797 → 14.719 | 5.469 → 5.469 |
| 14/0: `scenarios/shortest-path/variant-02/04-after-forces-extension.lp` | default source | 6.236 [6.186, 6.266] | 6.228 [6.191, 6.271] | 4.673 [4.656, 4.687] | 4.668 [4.653, 4.696] | 14.766 → 14.625 | 5.484 → 5.484 |
| 15/0: `scenarios/shortest-path/variant-02/05-ordering-unsat.lp` | default source | 6.240 [6.191, 6.274] | 6.235 [6.213, 6.266] | 4.671 [4.650, 4.694] | 4.682 [4.636, 4.693] | 14.562 → 14.594 | 5.391 → 5.391 |
| 16/0: `scenarios/shortest-path/variant-02/06-layered-dag-before.lp` | default source | 9.245 [9.225, 9.288] | 9.237 [9.218, 9.301] | 6.153 [4.673, 6.212] | 4.679 [4.653, 6.201] | 17.656 → 17.391 | 5.688 → 5.688 |
| 17/0: `scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp` | default source | 9.267 [9.237, 10.367] | 9.282 [9.265, 9.330] | 6.178 [4.655, 6.207] | 6.186 [4.635, 6.189] | 17.531 → 17.453 | 5.688 → 5.688 |
| 18/0: `scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp` | default source | 6.290 [6.239, 7.812] | 6.260 [6.205, 7.827] | 4.670 [4.658, 4.700] | 4.678 [4.657, 4.699] | 14.906 → 14.875 | 5.469 → 5.469 |
| 19/0: `scenarios/shortest-path/variant-02/09-negative-weights.lp` | default source | 6.211 [6.192, 7.764] | 6.218 [6.196, 6.258] | 4.659 [4.642, 4.676] | 4.670 [4.634, 4.685] | 14.984 → 14.922 | 5.422 → 5.422 |
| 20/0: `scenarios/shortest-path/variant-03/01-basic.lp` | default source | 6.259 [6.199, 7.794] | 6.251 [6.186, 7.864] | 4.680 [4.660, 4.696] | 4.675 [4.650, 4.685] | 14.750 → 14.703 | 5.453 → 5.453 |
| 21/0: `scenarios/shortest-path/variant-03/02-start-equals-end.lp` | default source | 6.247 [6.194, 7.772] | 6.255 [6.224, 6.263] | 4.685 [4.653, 4.693] | 4.671 [4.634, 4.694] | 14.719 → 14.688 | 5.391 → 5.391 |
| 22/0: `scenarios/shortest-path/variant-03/03-budget-forces-detour.lp` | default source | 6.233 [6.183, 6.273] | 6.254 [6.197, 6.268] | 4.676 [4.662, 4.694] | 4.673 [4.646, 4.704] | 14.750 → 14.672 | 5.438 → 5.438 |
| 23/0: `scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp` | default source | 6.221 [6.181, 6.294] | 6.239 [6.194, 6.268] | 4.675 [4.644, 4.688] | 4.688 [4.645, 4.706] | 14.656 → 14.609 | 5.391 → 5.391 |
| 24/0: `scenarios/shortest-path/variant-03/05-budget-unsat.lp` | default source | 6.286 [6.203, 7.798] | 6.248 [6.206, 7.795] | 4.664 [4.645, 4.695] | 4.660 [4.638, 4.688] | 14.891 → 14.891 | 5.516 → 5.516 |
| 25/0: `scenarios/shortest-path/variant-03/06-layered-dag-cap.lp` | default source | 10.675 [10.563, 10.787] | 10.573 [10.388, 10.775] | 6.198 [6.183, 6.224] | 6.190 [6.173, 6.208] | 19.000 → 18.891 | 5.938 → 5.938 |
| 26/0: `scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp` | default source | 12.288 [10.703, 12.363] | 12.286 [10.738, 12.378] | 6.179 [6.162, 6.221] | 6.183 [6.167, 6.201] | 20.516 → 23.188 | 5.938 → 5.938 |
| 27/0: `scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp` | default source | 7.797 [6.209, 7.821] | 7.782 [6.216, 7.841] | 4.646 [4.630, 4.688] | 4.664 [4.651, 4.727] | 14.875 → 14.906 | 5.484 → 5.484 |
| 28/0: `scenarios/shortest-path/variant-03/09-negative-weights.lp` | default source | 6.244 [6.181, 7.804] | 7.780 [6.197, 7.877] | 4.664 [4.647, 4.682] | 4.667 [4.636, 4.693] | 15.094 → 14.984 | 5.422 → 5.422 |
| 29/0: `scenarios/shortest-path/variant-04/01-basic.lp` | default source | 6.212 [6.193, 7.812] | 6.224 [6.193, 7.806] | 4.659 [4.647, 4.698] | 4.672 [4.638, 4.699] | 14.859 → 14.750 | 5.500 → 5.500 |
| 30/0: `scenarios/shortest-path/variant-04/02-start-equals-end.lp` | default source | 6.239 [6.194, 7.816] | 6.253 [6.199, 6.271] | 4.673 [4.653, 4.691] | 4.684 [4.653, 4.689] | 14.766 → 14.750 | 5.453 → 5.453 |
| 31/0: `scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp` | default source | 6.234 [6.209, 7.720] | 6.221 [6.210, 7.760] | 4.668 [4.643, 4.702] | 4.651 [4.646, 4.681] | 14.953 → 14.922 | 5.500 → 5.500 |
| 32/0: `scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp` | default source | 6.253 [6.180, 7.811] | 6.263 [6.190, 7.788] | 4.666 [4.654, 4.696] | 4.662 [4.634, 4.675] | 14.875 → 14.859 | 5.484 → 5.484 |
| 33/0: `scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp` | default source | 7.731 [6.213, 7.795] | 7.717 [6.233, 7.787] | 4.654 [4.336, 4.692] | 4.656 [4.625, 4.689] | 15.094 → 15.016 | 5.469 → 5.469 |
| 34/0: `scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp` | default source | 10.786 [10.747, 12.505] | 10.787 [10.725, 10.816] | 6.176 [6.155, 6.210] | 6.165 [6.158, 6.206] | 19.516 → 19.562 | 6.000 → 6.000 |
| 35/0: `scenarios/shortest-path/variant-04/07-layered-dag-combined.lp` | default source | 10.794 [10.724, 12.382] | 10.789 [10.759, 10.828] | 6.176 [6.155, 6.199] | 6.183 [6.175, 6.202] | 19.406 → 19.594 | 6.000 → 6.000 |
| 36/0: `scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp` | default source | 7.809 [7.719, 7.894] | 7.775 [7.721, 7.829] | 4.652 [4.637, 4.694] | 4.650 [4.633, 4.670] | 15.125 → 15.062 | 5.531 → 5.531 |
| 37/0: `scenarios/shortest-path/variant-04/09-negative-weights.lp` | default source | 7.709 [7.366, 7.801] | 7.717 [6.209, 7.787] | 4.649 [4.633, 4.679] | 4.654 [4.633, 4.685] | 15.234 → 15.156 | 5.469 → 5.469 |
| 38/0: `scenarios/task-allocation/variant-01/01-basic.lp` | default source | 6.275 [6.247, 6.290] | 6.260 [6.214, 6.279] | 4.678 [4.655, 4.689] | 4.663 [4.650, 4.688] | 14.047 → 14.062 | 5.297 → 5.297 |
| 39/0: `scenarios/task-allocation/variant-01/02-agent-reuse.lp` | default source | 6.242 [6.195, 6.275] | 6.240 [6.210, 6.269] | 4.670 [4.642, 4.710] | 4.658 [4.645, 4.688] | 14.328 → 14.297 | 5.359 → 5.359 |
| 40/0: `scenarios/task-allocation/variant-01/03-selective-compatibility.lp` | default source | 6.262 [6.182, 6.293] | 6.267 [6.223, 6.303] | 4.671 [4.655, 4.678] | 4.673 [4.649, 4.690] | 14.109 → 14.047 | 5.312 → 5.312 |
| 41/0: `scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp` | default source | 6.246 [6.202, 6.285] | 6.255 [5.918, 6.555] | 4.681 [4.650, 4.684] | 4.676 [4.644, 4.681] | 13.844 → 13.844 | 5.188 → 5.188 |
| 42/0: `scenarios/task-allocation/variant-01/05-larger-mix.lp` | default source | 6.229 [6.142, 7.733] | 6.223 [6.178, 6.246] | 4.677 [4.642, 4.694] | 4.678 [4.647, 4.695] | 14.875 → 14.844 | 5.359 → 5.359 |
| 43/0: `scenarios/task-allocation/variant-02/01-basic.lp` | default source | 6.218 [6.200, 6.265] | 6.223 [6.181, 7.786] | 4.670 [4.637, 4.682] | 4.663 [4.643, 4.689] | 14.406 → 14.484 | 5.453 → 5.453 |
| 44/0: `scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp` | default source | 6.252 [6.196, 7.788] | 6.241 [6.197, 7.776] | 4.678 [4.624, 4.685] | 4.677 [4.663, 4.684] | 14.609 → 14.656 | 5.453 → 5.453 |
| 45/0: `scenarios/task-allocation/variant-02/03-cost-dominates.lp` | default source | 6.226 [6.184, 7.795] | 7.719 [6.190, 7.805] | 4.665 [4.627, 4.690] | 4.673 [4.643, 4.687] | 14.781 → 14.906 | 5.453 → 5.453 |
| 46/0: `scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp` | default source | 6.258 [6.177, 7.841] | 6.271 [6.184, 6.275] | 4.679 [4.670, 4.687] | 4.671 [4.306, 4.693] | 14.375 → 14.359 | 5.281 → 5.281 |
| 47/0: `scenarios/task-allocation/variant-02/05-larger-mix.lp` | default source | 9.187 [8.881, 10.522] | 9.227 [8.817, 9.268] | 4.679 [4.650, 6.260] | 4.668 [4.652, 4.708] | 16.156 → 16.203 | 5.562 → 5.562 |
| 48/0: `scenarios/task-allocation/variant-03/01-basic.lp` | default source | 6.264 [6.240, 7.839] | 6.270 [6.232, 6.300] | 4.658 [4.650, 4.686] | 4.655 [4.645, 4.678] | 14.250 → 14.281 | 5.391 → 5.391 |
| 49/0: `scenarios/task-allocation/variant-03/02-multiple-groups.lp` | default source | 6.236 [6.193, 7.793] | 6.218 [6.184, 7.807] | 4.675 [4.646, 4.687] | 4.661 [4.627, 4.713] | 14.578 → 14.453 | 5.391 → 5.391 |
| 50/0: `scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp` | default source | 6.245 [6.205, 7.855] | 6.237 [6.224, 6.276] | 4.660 [4.641, 4.691] | 4.662 [4.650, 4.693] | 14.422 → 14.359 | 5.391 → 5.391 |
| 51/0: `scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp` | default source | 6.284 [5.943, 7.783] | 6.238 [6.223, 6.328] | 4.672 [4.644, 4.707] | 4.676 [4.646, 4.689] | 14.047 → 14.094 | 5.297 → 5.297 |
| 52/0: `scenarios/task-allocation/variant-03/05-larger-mix.lp` | default source | 7.758 [6.187, 7.828] | 6.226 [6.188, 7.764] | 4.667 [4.654, 4.685] | 4.676 [4.615, 4.701] | 14.734 → 14.734 | 5.391 → 5.391 |
| 53/0: `scenarios/task-allocation/variant-04/01-basic.lp` | default source | 10.727 [10.388, 11.893] | 9.236 [9.205, 10.463] | 22.738 [21.264, 22.818] | 22.743 [22.717, 22.793] | 16.016 → 15.969 | 8.797 → 8.797 |
| 54/0: `scenarios/task-allocation/variant-04/02-precedence.lp` | default source | 7.789 [7.695, 9.363] | 7.779 [7.679, 9.459] | 6.166 [6.161, 6.235] | 6.171 [6.152, 6.292] | 15.703 → 15.547 | 5.828 → 5.828 |
| 55/0: `scenarios/task-allocation/variant-04/03-agent-serialization.lp` | default source | 9.251 [8.826, 10.846] | 9.288 [9.258, 9.318] | 24.232 [22.721, 28.177] | 24.207 [22.715, 24.249] | 15.703 → 15.719 | 8.000 → 8.000 |
| 56/0: `scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp` | default source | 7.756 [7.332, 7.888] | 7.782 [6.200, 134.550] | 4.656 [4.637, 6.306] | 4.655 [4.633, 4.684] | 15.172 → 15.094 | 5.547 → 5.547 |
| 57/0: `scenarios/task-allocation/variant-04/05-larger-mix.lp` | default source | 36.840 [36.528, 38.270] | 35.228 [34.869, 40.964] | 180.831 [163.763, 188.575] | 180.973 [161.340, 188.516] | 22.641 → 22.484 | 19.406 → 19.406 |
| 58/0: `scenarios/traveling-salesman/variant-01/01-basic.lp` | default source | 7.883 [7.865, 8.000] | 7.899 [7.514, 7.994] | 4.702 [4.652, 4.772] | 4.705 [4.649, 4.785] | 14.391 → 14.516 | 5.484 → 5.484 |
| 59/0: `scenarios/traveling-salesman/variant-01/02-multiple-tours.lp` | default source | 7.728 [6.191, 7.818] | 7.774 [6.183, 7.803] | 4.663 [4.394, 4.705] | 4.668 [4.644, 4.697] | 15.188 → 15.250 | 5.531 → 5.531 |
| 60/0: `scenarios/traveling-salesman/variant-01/03-asymmetric.lp` | default source | 7.781 [7.653, 7.818] | 7.759 [7.315, 7.807] | 4.651 [4.639, 4.696] | 4.646 [4.431, 4.667] | 15.703 → 15.703 | 5.609 → 5.609 |
| 61/0: `scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp` | default source | 6.249 [6.225, 6.291] | 6.241 [6.207, 6.284] | 4.658 [4.644, 4.679] | 4.658 [4.649, 4.694] | 14.188 → 14.172 | 5.266 → 5.266 |
| 62/0: `scenarios/traveling-salesman/variant-01/05-ring.lp` | default source | 9.242 [8.836, 9.285] | 9.225 [8.779, 9.249] | 4.673 [4.638, 4.687] | 4.675 [4.631, 4.687] | 16.594 → 16.609 | 5.609 → 5.609 |
| 63/0: `scenarios/traveling-salesman/variant-02/01-basic.lp` | default source | 6.225 [6.206, 7.794] | 7.743 [6.199, 7.793] | 4.646 [4.642, 4.668] | 4.653 [4.628, 4.676] | 14.656 → 14.719 | 5.500 → 5.500 |
| 64/0: `scenarios/traveling-salesman/variant-02/02-single-salesman.lp` | default source | 6.253 [6.180, 6.272] | 6.239 [5.921, 7.763] | 4.667 [4.647, 4.673] | 4.672 [4.664, 4.688] | 14.672 → 14.672 | 5.438 → 5.438 |
| 65/0: `scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp` | default source | 6.234 [6.200, 7.372] | 6.210 [6.183, 6.259] | 4.679 [4.655, 4.686] | 4.668 [4.660, 4.683] | 14.594 → 14.531 | 5.406 → 5.391 |
| 66/0: `scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp` | default source | 6.221 [6.176, 6.263] | 6.227 [6.174, 7.791] | 4.682 [4.639, 4.706] | 4.678 [4.660, 4.700] | 14.594 → 14.688 | 5.500 → 5.500 |
| 67/0: `scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp` | default source | 7.732 [6.191, 7.796] | 6.251 [6.189, 7.794] | 4.666 [4.629, 4.684] | 4.675 [4.646, 4.704] | 14.828 → 14.766 | 5.484 → 5.484 |
| 68/0: `scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp` | default source | 7.778 [6.210, 7.807] | 7.758 [7.692, 7.819] | 4.675 [4.639, 4.691] | 4.675 [4.652, 4.690] | 15.047 → 14.938 | 5.484 → 5.484 |
| 69/0: `scenarios/traveling-salesman/variant-03/01-basic.lp` | default source | 6.247 [6.187, 7.311] | 6.240 [6.191, 7.573] | 4.665 [4.649, 4.701] | 4.670 [4.658, 4.687] | 14.672 → 14.641 | 5.453 → 5.453 |
| 70/0: `scenarios/traveling-salesman/variant-03/02-single-salesman.lp` | default source | 6.217 [6.201, 6.263] | 6.231 [6.194, 6.281] | 4.675 [4.667, 4.699] | 4.683 [4.660, 4.705] | 14.703 → 14.562 | 5.453 → 5.453 |
| 71/0: `scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp` | default source | 6.229 [6.181, 6.279] | 6.233 [6.194, 6.276] | 4.678 [4.640, 4.686] | 4.672 [4.659, 4.704] | 14.594 → 14.500 | 5.406 → 5.406 |
| 72/0: `scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp` | default source | 6.246 [6.224, 7.764] | 7.758 [6.198, 7.782] | 4.657 [4.641, 4.691] | 4.684 [4.639, 4.701] | 14.797 → 14.844 | 5.516 → 5.516 |
| 73/0: `scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp` | default source | 7.748 [7.220, 7.790] | 7.756 [7.678, 7.808] | 4.668 [4.655, 4.683] | 4.664 [4.649, 4.676] | 15.047 → 15.078 | 5.484 → 5.484 |
| 74/0: `scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp` | default source | 7.755 [7.707, 7.825] | 7.761 [7.693, 7.824] | 4.675 [4.668, 4.682] | 4.664 [4.656, 4.680] | 14.875 → 14.859 | 5.438 → 5.438 |
| 75/0: `scenarios/traveling-salesman/variant-04/01-basic.lp` | default source | 7.756 [7.706, 7.859] | 7.742 [7.710, 7.773] | 4.671 [4.639, 4.686] | 4.671 [4.634, 4.683] | 15.281 → 15.312 | 5.562 → 5.562 |
| 76/0: `scenarios/traveling-salesman/variant-04/02-single-salesman.lp` | default source | 7.769 [7.707, 7.800] | 7.759 [7.720, 7.833] | 4.656 [4.646, 4.682] | 4.654 [4.631, 4.663] | 15.141 → 15.141 | 5.578 → 5.578 |
| 77/0: `scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp` | default source | 7.743 [7.707, 7.820] | 7.745 [7.700, 7.838] | 4.654 [4.651, 4.679] | 4.659 [4.655, 4.673] | 15.031 → 15.016 | 5.531 → 5.531 |
| 78/0: `scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp` | default source | 6.243 [6.197, 7.744] | 7.759 [6.188, 7.790] | 4.652 [4.643, 4.662] | 4.657 [4.649, 4.689] | 14.953 → 14.953 | 5.516 → 5.516 |
| 79/0: `scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp` | default source | 9.219 [9.200, 9.256] | 9.226 [9.172, 9.240] | 4.650 [4.628, 6.197] | 4.656 [4.426, 4.685] | 15.609 → 15.594 | 5.578 → 5.578 |
| 80/0: `scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp` | default source | 9.263 [9.052, 9.355] | 9.296 [9.239, 9.323] | 4.632 [4.623, 4.758] | 4.647 [4.633, 4.679] | 15.609 → 15.641 | 5.578 → 5.578 |
| 81/0: `scenarios/traveling-salesman/variant-05/01-basic.lp` | default source | 7.756 [7.719, 7.795] | 7.744 [7.716, 7.806] | 4.646 [4.635, 4.658] | 4.640 [4.630, 4.676] | 15.328 → 15.297 | 5.578 → 5.578 |
| 82/0: `scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp` | default source | 7.780 [7.709, 7.811] | 7.748 [7.571, 7.795] | 4.649 [4.626, 4.671] | 4.637 [4.625, 4.686] | 15.141 → 15.141 | 5.562 → 5.562 |
| 83/0: `scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp` | default source | 7.743 [7.704, 7.803] | 7.776 [7.724, 7.917] | 4.651 [4.646, 4.681] | 4.668 [4.653, 4.675] | 15.078 → 15.031 | 5.531 → 5.531 |
| 84/0: `scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp` | default source | 7.760 [7.702, 7.823] | 7.775 [7.724, 7.794] | 4.656 [4.634, 4.671] | 4.660 [4.646, 4.766] | 15.125 → 15.219 | 5.578 → 5.578 |
| 85/0: `scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp` | default source | 9.231 [9.209, 9.294] | 9.234 [9.191, 9.286] | 4.654 [4.635, 4.672] | 4.652 [4.634, 4.677] | 15.688 → 15.734 | 5.594 → 5.594 |
| 86/0: `scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp` | default source | 9.286 [9.246, 9.302] | 9.267 [9.243, 9.306] | 4.639 [4.634, 4.658] | 4.639 [4.619, 4.665] | 15.672 → 15.672 | 5.578 → 5.578 |
| 87/0: `standalone/n-queens/variant-01.lp` | n=8 | 15.068 [14.782, 15.255] | 13.815 [13.264, 13.858] | 6.179 [4.642, 6.199] | 6.170 [4.635, 6.191] | 15.172 → 15.047 | 5.422 → 5.422 |
| 88/0: `standalone/n-queens/variant-02.lp` | n=8 | 54.472 [53.454, 55.241] | 54.059 [53.088, 54.782] | 117.551 [114.538, 118.794] | 119.080 [117.248, 119.116] | 21.734 → 21.266 | 8.781 → 8.312 |
| 89/0: `standalone/n-queens/variant-03.lp` | n=8 | 13.751 [13.580, 13.902] | 13.562 [13.345, 13.938] | 6.200 [6.186, 6.234] | 6.198 [6.179, 6.218] | 15.344 → 15.406 | 5.438 → 5.438 |
| 90/0: `standalone/n-queens/variant-04.lp` | n=8 | 9.258 [8.922, 9.275] | 9.111 [8.827, 9.331] | 6.181 [6.170, 6.210] | 6.170 [6.157, 6.191] | 14.984 → 15.094 | 5.469 → 5.469 |
| 91/0: `standalone/n-queens/variant-05.lp` | n=8 | 9.259 [9.122, 9.276] | 9.109 [8.921, 9.281] | 6.181 [6.165, 6.260] | 6.219 [6.169, 6.247] | 15.688 → 15.688 | 5.531 → 5.531 |
| 92/0: `standalone/n-queens/variant-06.lp` | n=8 | 10.651 [9.192, 11.930] | 10.812 [9.158, 10.847] | 6.186 [6.160, 6.210] | 6.198 [6.178, 6.229] | 15.750 → 15.812 | 5.562 → 5.562 |
| 93/0: `standalone/send-money/send-money.lp` | default source | 15.041 [13.575, 15.423] | 14.988 [13.651, 15.375] | 12.190 [10.661, 12.207] | 12.195 [12.178, 12.204] | 19.125 → 19.188 | 8.281 → 8.281 |

## Grounding, solving and driver intervals

Median milliseconds, baseline → candidate. Native grounding/solving use the recorded typed stages; driver uses the maintained comparison. Lazy grounding is interleaved with solving, so its separate grounding stage is unavailable. Native driver excludes source loading and statistics output. Clingo non-solving is its reported Total−Solve, including grounding and preprocessing; it is not independently measured pure grounding. These scopes differ from process wall time. Fine source phases retained in the evidence JSON are nested intervals and must not be added to these totals.

| Case/profile and complete entry | Parameters | Native grounding B → C | Native solving B → C | Native driver B → C | Clingo non-solving B → C | Clingo solving B → C |
|---|---|---:|---:|---:|---:|---:|
| 0/0: `scenarios/equality-generalized-tsp/01-basic.lp` | default source | 0.637 → 0.605 | 0.285 → 0.268 | 1.717 → 1.647 | 1.000 → 1.000 | 0.000 → 0.000 |
| 1/0: `scenarios/equality-generalized-tsp/02-larger.lp` | default source | 1.037 → 0.993 | 0.403 → 0.388 | 2.278 → 2.223 | 1.000 → 1.000 | 0.000 → 0.000 |
| 2/0: `scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp` | default source | 0.321 → 0.308 | 0.123 → 0.112 | 1.104 → 1.085 | 0.000 → 0.000 | 0.000 → 0.000 |
| 3/0: `scenarios/shortest-path/variant-01/01-basic.lp` | default source | 0.320 → 0.318 | 0.157 → 0.137 | 1.185 → 1.190 | 1.000 → 1.000 | 0.000 → 0.000 |
| 4/0: `scenarios/shortest-path/variant-01/02-start-equals-end.lp` | default source | 0.274 → 0.271 | 0.142 → 0.124 | 1.110 → 1.098 | 1.000 → 1.000 | 0.000 → 0.000 |
| 5/0: `scenarios/shortest-path/variant-01/03-zero-cost-detour.lp` | default source | 0.296 → 0.302 | 0.153 → 0.132 | 1.151 → 1.151 | 1.000 → 1.000 | 0.000 → 0.000 |
| 6/0: `scenarios/shortest-path/variant-01/04-no-path.lp` | default source | 0.280 → 0.277 | 0.106 → 0.101 | 1.080 → 1.079 | 1.000 → 1.000 | 0.000 → 0.000 |
| 7/0: `scenarios/shortest-path/variant-01/05-multi-path.lp` | default source | 0.512 → 0.497 | 0.228 → 0.212 | 1.486 → 1.465 | 1.000 → 1.000 | 0.000 → 0.000 |
| 8/0: `scenarios/shortest-path/variant-01/06-layered-dag.lp` | default source | 1.491 → 1.454 | 1.220 → 1.206 | 3.721 → 3.686 | 1.000 → 1.000 | 0.000 → 0.000 |
| 9/0: `scenarios/shortest-path/variant-01/07-cycles.lp` | default source | 0.662 → 0.647 | 0.329 → 0.312 | 1.827 → 1.829 | 1.000 → 1.000 | 0.000 → 0.000 |
| 10/0: `scenarios/shortest-path/variant-01/08-negative-weights.lp` | default source | 0.426 → 0.423 | 0.237 → 0.222 | 1.407 → 1.414 | 1.000 → 1.000 | 0.000 → 0.000 |
| 11/0: `scenarios/shortest-path/variant-02/01-basic.lp` | default source | 0.341 → 0.329 | 0.159 → 0.138 | 1.315 → 1.290 | 1.000 → 1.000 | 0.000 → 0.000 |
| 12/0: `scenarios/shortest-path/variant-02/02-start-equals-end.lp` | default source | 0.304 → 0.300 | 0.151 → 0.132 | 1.252 → 1.235 | 1.000 → 1.000 | 0.000 → 0.000 |
| 13/0: `scenarios/shortest-path/variant-02/03-before-forces-detour.lp` | default source | 0.414 → 0.399 | 0.172 → 0.157 | 1.420 → 1.401 | 1.000 → 1.000 | 0.000 → 0.000 |
| 14/0: `scenarios/shortest-path/variant-02/04-after-forces-extension.lp` | default source | 0.402 → 0.390 | 0.169 → 0.151 | 1.402 → 1.396 | 1.000 → 1.000 | 0.000 → 0.000 |
| 15/0: `scenarios/shortest-path/variant-02/05-ordering-unsat.lp` | default source | 0.322 → 0.320 | 0.116 → 0.109 | 1.224 → 1.234 | 1.000 → 1.000 | 0.000 → 0.000 |
| 16/0: `scenarios/shortest-path/variant-02/06-layered-dag-before.lp` | default source | 1.620 → 1.586 | 1.420 → 1.408 | 4.192 → 4.157 | 2.000 → 2.000 | 0.000 → 0.000 |
| 17/0: `scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp` | default source | 1.633 → 1.580 | 1.198 → 1.194 | 3.981 → 3.962 | 2.000 → 2.000 | 0.000 → 0.000 |
| 18/0: `scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp` | default source | 0.431 → 0.423 | 0.216 → 0.190 | 1.531 → 1.480 | 1.000 → 1.000 | 0.000 → 0.000 |
| 19/0: `scenarios/shortest-path/variant-02/09-negative-weights.lp` | default source | 0.475 → 0.462 | 0.252 → 0.237 | 1.560 → 1.553 | 1.000 → 1.000 | 0.000 → 0.000 |
| 20/0: `scenarios/shortest-path/variant-03/01-basic.lp` | default source | 0.371 → 0.365 | 0.154 → 0.135 | 1.344 → 1.321 | 1.000 → 1.000 | 0.000 → 0.000 |
| 21/0: `scenarios/shortest-path/variant-03/02-start-equals-end.lp` | default source | 0.332 → 0.313 | 0.149 → 0.131 | 1.279 → 1.250 | 1.000 → 1.000 | 0.000 → 0.000 |
| 22/0: `scenarios/shortest-path/variant-03/03-budget-forces-detour.lp` | default source | 0.394 → 0.390 | 0.162 → 0.144 | 1.371 → 1.359 | 1.000 → 1.000 | 0.000 → 0.000 |
| 23/0: `scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp` | default source | 0.358 → 0.351 | 0.140 → 0.126 | 1.293 → 1.294 | 1.000 → 1.000 | 0.000 → 0.000 |
| 24/0: `scenarios/shortest-path/variant-03/05-budget-unsat.lp` | default source | 0.635 → 0.618 | 0.197 → 0.187 | 1.680 → 1.671 | 1.000 → 1.000 | 0.000 → 0.000 |
| 25/0: `scenarios/shortest-path/variant-03/06-layered-dag-cap.lp` | default source | 2.338 → 2.269 | 2.151 → 2.153 | 5.688 → 5.625 | 2.000 → 2.000 | 0.000 → 0.000 |
| 26/0: `scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp` | default source | 2.365 → 2.272 | 2.489 → 2.515 | 6.106 → 6.031 | 2.000 → 2.000 | 0.000 → 0.000 |
| 27/0: `scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp` | default source | 0.506 → 0.493 | 0.222 → 0.202 | 1.606 → 1.590 | 1.000 → 1.000 | 0.000 → 0.000 |
| 28/0: `scenarios/shortest-path/variant-03/09-negative-weights.lp` | default source | 0.533 → 0.519 | 0.257 → 0.235 | 1.645 → 1.630 | 1.000 → 1.000 | 0.000 → 0.000 |
| 29/0: `scenarios/shortest-path/variant-04/01-basic.lp` | default source | 0.394 → 0.385 | 0.162 → 0.143 | 1.473 → 1.443 | 1.000 → 1.000 | 0.000 → 0.000 |
| 30/0: `scenarios/shortest-path/variant-04/02-start-equals-end.lp` | default source | 0.371 → 0.361 | 0.159 → 0.140 | 1.447 → 1.413 | 1.000 → 1.000 | 0.000 → 0.000 |
| 31/0: `scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp` | default source | 0.504 → 0.499 | 0.182 → 0.164 | 1.633 → 1.614 | 1.000 → 1.000 | 0.000 → 0.000 |
| 32/0: `scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp` | default source | 0.516 → 0.499 | 0.152 → 0.147 | 1.611 → 1.596 | 1.000 → 1.000 | 0.000 → 0.000 |
| 33/0: `scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp` | default source | 0.565 → 0.544 | 0.227 → 0.208 | 1.746 → 1.726 | 1.000 → 1.000 | 0.000 → 0.000 |
| 34/0: `scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp` | default source | 2.589 → 2.468 | 1.712 → 1.717 | 5.627 → 5.515 | 2.000 → 2.000 | 0.000 → 0.000 |
| 35/0: `scenarios/shortest-path/variant-04/07-layered-dag-combined.lp` | default source | 2.616 → 2.463 | 1.817 → 1.801 | 5.814 → 5.643 | 2.000 → 2.000 | 0.000 → 0.000 |
| 36/0: `scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp` | default source | 0.783 → 0.752 | 0.301 → 0.279 | 2.126 → 2.088 | 1.000 → 1.000 | 0.000 → 0.000 |
| 37/0: `scenarios/shortest-path/variant-04/09-negative-weights.lp` | default source | 0.593 → 0.568 | 0.264 → 0.245 | 1.805 → 1.786 | 1.000 → 1.000 | 0.000 → 0.000 |
| 38/0: `scenarios/task-allocation/variant-01/01-basic.lp` | default source | 0.238 → 0.231 | 0.172 → 0.149 | 0.923 → 0.914 | 0.000 → 0.000 | 0.000 → 0.000 |
| 39/0: `scenarios/task-allocation/variant-01/02-agent-reuse.lp` | default source | 0.279 → 0.274 | 0.290 → 0.257 | 1.105 → 1.092 | 0.000 → 0.000 | 0.000 → 0.000 |
| 40/0: `scenarios/task-allocation/variant-01/03-selective-compatibility.lp` | default source | 0.275 → 0.271 | 0.188 → 0.168 | 0.999 → 0.987 | 0.000 → 0.000 | 0.000 → 0.000 |
| 41/0: `scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp` | default source | 0.216 → 0.212 | 0.110 → 0.101 | 0.804 → 0.799 | 0.000 → 0.000 | 0.000 → 0.000 |
| 42/0: `scenarios/task-allocation/variant-01/05-larger-mix.lp` | default source | 0.421 → 0.406 | 0.572 → 0.560 | 1.626 → 1.610 | 0.000 → 0.000 | 0.000 → 0.000 |
| 43/0: `scenarios/task-allocation/variant-02/01-basic.lp` | default source | 0.560 → 0.552 | 0.217 → 0.201 | 1.472 → 1.457 | 1.000 → 1.000 | 0.000 → 0.000 |
| 44/0: `scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp` | default source | 0.648 → 0.629 | 0.262 → 0.243 | 1.673 → 1.642 | 1.000 → 1.000 | 0.000 → 0.000 |
| 45/0: `scenarios/task-allocation/variant-02/03-cost-dominates.lp` | default source | 0.638 → 0.627 | 0.341 → 0.311 | 1.732 → 1.715 | 1.000 → 1.000 | 0.000 → 0.000 |
| 46/0: `scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp` | default source | 0.505 → 0.497 | 0.140 → 0.133 | 1.321 → 1.332 | 1.000 → 0.000 | 0.000 → 0.000 |
| 47/0: `scenarios/task-allocation/variant-02/05-larger-mix.lp` | default source | 1.366 → 1.336 | 2.064 → 2.027 | 4.373 → 4.321 | 1.000 → 1.000 | 0.000 → 0.000 |
| 48/0: `scenarios/task-allocation/variant-03/01-basic.lp` | default source | 0.266 → 0.257 | 0.165 → 0.142 | 1.081 → 1.056 | 0.000 → 0.000 | 0.000 → 0.000 |
| 49/0: `scenarios/task-allocation/variant-03/02-multiple-groups.lp` | default source | 0.380 → 0.366 | 0.234 → 0.215 | 1.341 → 1.323 | 1.000 → 0.000 | 0.000 → 0.000 |
| 50/0: `scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp` | default source | 0.318 → 0.312 | 0.197 → 0.181 | 1.192 → 1.183 | 0.000 → 0.000 | 0.000 → 0.000 |
| 51/0: `scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp` | default source | 0.225 → 0.213 | 0.115 → 0.107 | 0.958 → 0.927 | 0.000 → 0.000 | 0.000 → 0.000 |
| 52/0: `scenarios/task-allocation/variant-03/05-larger-mix.lp` | default source | 0.457 → 0.440 | 0.355 → 0.325 | 1.576 → 1.538 | 1.000 → 1.000 | 0.000 → 0.000 |
| 53/0: `scenarios/task-allocation/variant-04/01-basic.lp` | default source | 2.135 → 1.724 | 1.503 → 1.478 | 4.998 → 4.567 | 14.000 → 14.000 | 4.000 → 4.000 |
| 54/0: `scenarios/task-allocation/variant-04/02-precedence.lp` | default source | 1.180 → 1.020 | 0.421 → 0.404 | 2.835 → 2.666 | 2.000 → 2.000 | 0.000 → 0.000 |
| 55/0: `scenarios/task-allocation/variant-04/03-agent-serialization.lp` | default source | 2.008 → 1.591 | 0.791 → 0.772 | 4.006 → 3.567 | 20.000 → 20.000 | 0.000 → 0.000 |
| 56/0: `scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp` | default source | 0.539 → 0.516 | 0.162 → 0.151 | 1.783 → 1.755 | 1.000 → 1.000 | 0.000 → 0.000 |
| 57/0: `scenarios/task-allocation/variant-04/05-larger-mix.lp` | default source | 6.893 → 5.277 | 12.862 → 12.799 | 31.236 → 30.029 | 70.000 → 69.000 | 106.000 → 104.000 |
| 58/0: `scenarios/traveling-salesman/variant-01/01-basic.lp` | default source | 0.478 → 0.464 | 0.235 → 0.216 | 1.463 → 1.439 | 1.000 → 1.000 | 0.000 → 0.000 |
| 59/0: `scenarios/traveling-salesman/variant-01/02-multiple-tours.lp` | default source | 0.643 → 0.617 | 0.531 → 0.521 | 1.893 → 1.882 | 1.000 → 1.000 | 0.000 → 0.000 |
| 60/0: `scenarios/traveling-salesman/variant-01/03-asymmetric.lp` | default source | 0.776 → 0.741 | 0.737 → 0.723 | 2.239 → 2.222 | 1.000 → 1.000 | 0.000 → 0.000 |
| 61/0: `scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp` | default source | 0.346 → 0.339 | 0.130 → 0.110 | 1.084 → 1.062 | 0.000 → 0.000 | 0.000 → 0.000 |
| 62/0: `scenarios/traveling-salesman/variant-01/05-ring.lp` | default source | 0.935 → 0.902 | 1.665 → 1.683 | 3.446 → 3.432 | 1.000 → 1.000 | 0.000 → 0.000 |
| 63/0: `scenarios/traveling-salesman/variant-02/01-basic.lp` | default source | 0.616 → 0.582 | 0.235 → 0.220 | 1.643 → 1.616 | 1.000 → 1.000 | 0.000 → 0.000 |
| 64/0: `scenarios/traveling-salesman/variant-02/02-single-salesman.lp` | default source | 0.417 → 0.401 | 0.217 → 0.198 | 1.416 → 1.367 | 1.000 → 1.000 | 0.000 → 0.000 |
| 65/0: `scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp` | default source | 0.523 → 0.493 | 0.145 → 0.138 | 1.397 → 1.365 | 1.000 → 1.000 | 0.000 → 0.000 |
| 66/0: `scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp` | default source | 0.526 → 0.487 | 0.213 → 0.201 | 1.503 → 1.464 | 1.000 → 1.000 | 0.000 → 0.000 |
| 67/0: `scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp` | default source | 0.757 → 0.700 | 0.252 → 0.238 | 1.811 → 1.748 | 1.000 → 1.000 | 0.000 → 0.000 |
| 68/0: `scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp` | default source | 0.782 → 0.727 | 0.297 → 0.277 | 1.911 → 1.839 | 1.000 → 1.000 | 0.000 → 0.000 |
| 69/0: `scenarios/traveling-salesman/variant-03/01-basic.lp` | default source | 0.549 → 0.515 | 0.212 → 0.194 | 1.529 → 1.478 | 1.000 → 1.000 | 0.000 → 0.000 |
| 70/0: `scenarios/traveling-salesman/variant-03/02-single-salesman.lp` | default source | 0.406 → 0.390 | 0.212 → 0.194 | 1.375 → 1.354 | 1.000 → 1.000 | 0.000 → 0.000 |
| 71/0: `scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp` | default source | 0.527 → 0.492 | 0.142 → 0.132 | 1.400 → 1.368 | 1.000 → 1.000 | 0.000 → 0.000 |
| 72/0: `scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp` | default source | 0.689 → 0.650 | 0.268 → 0.250 | 1.768 → 1.734 | 1.000 → 1.000 | 0.000 → 0.000 |
| 73/0: `scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp` | default source | 1.363 → 1.223 | 0.371 → 0.351 | 2.605 → 2.469 | 1.000 → 1.000 | 0.000 → 0.000 |
| 74/0: `scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp` | default source | 0.857 → 0.786 | 0.286 → 0.272 | 1.965 → 1.891 | 1.000 → 1.000 | 0.000 → 0.000 |
| 75/0: `scenarios/traveling-salesman/variant-04/01-basic.lp` | default source | 1.087 → 1.017 | 0.267 → 0.250 | 2.507 → 2.428 | 1.000 → 1.000 | 0.000 → 0.000 |
| 76/0: `scenarios/traveling-salesman/variant-04/02-single-salesman.lp` | default source | 0.799 → 0.755 | 0.214 → 0.198 | 2.151 → 2.098 | 1.000 → 1.000 | 0.000 → 0.000 |
| 77/0: `scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp` | default source | 0.640 → 0.627 | 0.145 → 0.134 | 1.844 → 1.841 | 1.000 → 1.000 | 0.000 → 0.000 |
| 78/0: `scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp` | default source | 0.506 → 0.498 | 0.128 → 0.114 | 1.682 → 1.658 | 1.000 → 1.000 | 0.000 → 0.000 |
| 79/0: `scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp` | default source | 2.760 → 2.526 | 0.454 → 0.440 | 4.430 → 4.194 | 1.000 → 1.000 | 0.000 → 0.000 |
| 80/0: `scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp` | default source | 2.092 → 1.948 | 0.422 → 0.396 | 3.770 → 3.588 | 1.000 → 1.000 | 0.000 → 0.000 |
| 81/0: `scenarios/traveling-salesman/variant-05/01-basic.lp` | default source | 1.081 → 0.995 | 0.265 → 0.244 | 2.513 → 2.449 | 1.000 → 1.000 | 0.000 → 0.000 |
| 82/0: `scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp` | default source | 0.653 → 0.624 | 0.194 → 0.173 | 1.967 → 1.939 | 1.000 → 1.000 | 0.000 → 0.000 |
| 83/0: `scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp` | default source | 0.642 → 0.621 | 0.148 → 0.139 | 1.869 → 1.859 | 1.000 → 1.000 | 0.000 → 0.000 |
| 84/0: `scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp` | default source | 0.657 → 0.627 | 0.196 → 0.178 | 1.988 → 1.958 | 1.000 → 1.000 | 0.000 → 0.000 |
| 85/0: `scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp` | default source | 2.763 → 2.565 | 0.452 → 0.444 | 4.481 → 4.274 | 1.000 → 1.000 | 0.000 → 0.000 |
| 86/0: `scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp` | default source | 2.100 → 1.974 | 0.405 → 0.398 | 3.768 → 3.665 | 1.000 → 1.000 | 0.000 → 0.000 |
| 87/0: `standalone/n-queens/variant-01.lp` | n=8 | 7.583 → 5.719 | 1.811 → 1.848 | 10.169 → 8.329 | 1.000 → 1.000 | 1.000 → 1.000 |
| 88/0: `standalone/n-queens/variant-02.lp` | n=8 | 6.492 → 5.997 | 41.296 → 41.339 | 48.827 → 48.315 | 1.000 → 1.000 | 112.000 → 113.000 |
| 89/0: `standalone/n-queens/variant-03.lp` | n=8 | 5.701 → 5.432 | 1.913 → 1.924 | 8.512 → 8.215 | 1.000 → 1.000 | 1.000 → 1.000 |
| 90/0: `standalone/n-queens/variant-04.lp` | n=8 | 1.658 → 1.463 | 1.806 → 1.797 | 4.331 → 4.107 | 1.000 → 1.000 | 1.000 → 1.000 |
| 91/0: `standalone/n-queens/variant-05.lp` | n=8 | 1.449 → 1.446 | 1.656 → 1.705 | 4.395 → 4.404 | 1.000 → 1.000 | 1.000 → 1.000 |
| 92/0: `standalone/n-queens/variant-06.lp` | n=8 | 1.476 → 1.463 | 1.777 → 1.818 | 4.550 → 4.552 | 1.000 → 1.000 | 1.000 → 1.000 |
| 93/0: `standalone/send-money/send-money.lp` | default source | 4.809 → 4.920 | 2.611 → 2.617 | 8.761 → 8.837 | 7.000 → 7.000 | 1.000 → 1.000 |

All reported positions passed; all source inputs were unchanged. The full native qualification-family fingerprint matches before and after for every row. Clingo qualification compares displayed answers and costs; it cannot establish equality of hidden clingo interpretations. Its reported non-solving time includes preprocessing.
