% Scenario 02: Two independent groups.
% @expect sat
% @cost { 12 }
% @count optimal 4
%
% Four tasks in two groups, g1 = {t1,t2} and g2 = {t3,t4}; each group shares an
% agent internally, but the groups are independent. The symmetric cost matrix
% makes every group-to-agent combination cost the same: 6 per group, 12 overall,
% with four co-optimal assignments.

#include "../../../encodings/task-allocation/variant-03.lp".

task(t1; t2; t3; t4).
agent(a1; a2).

cost_of(
    a1,t1,1;
    a2,t1,5;
    a1,t2,5;
    a2,t2,1;
    a1,t3,1;
    a2,t3,5;
    a1,t4,5;
    a2,t4,1
).

in_group(
    t1,g1;
    t2,g1;
    t3,g2;
    t4,g2
).
