% Scenario 07: Layered DAG with before/2 and after/2 constraints.
% @expect sat
% @cost { 6 4 }
% @count optimal 1
% @optimal { start(s), included(a3,b3,1), included(b3,c3,1), included(c3,t,2), included(s,a3,2), end(t) }
%
% The variant-01 layered DAG with two ordering constraints. before(b3,t) forces
% the path through b3. after(c2,a2) is vacuous unless a2 is reached, and the
% optimizer avoids a2 (s -> a2 has weight 4 and a2 aids no feasible path), so the
% answer is dictated by before(b3,t) alone: s -> a3 -> b3 -> c3 -> t at weight 6.

#include "../../../encodings/shortest-path/variant-02.lp".

vertex(s; a1; a2; a3; b1; b2; b3; c1; c2; c3; t).

edge(
    s,a1,1;
    s,a2,4;
    s,a3,2;
    a1,b1,1;
    a1,b2,3;
    a1,b3,5;
    a2,b1,2;
    a2,b2,1;
    a2,b3,3;
    a3,b1,4;
    a3,b2,2;
    a3,b3,1;
    b1,c1,1;
    b1,c2,3;
    b1,c3,5;
    b2,c1,2;
    b2,c2,1;
    b2,c3,3;
    b3,c1,4;
    b3,c2,2;
    b3,c3,1;
    c1,t,1;
    c2,t,3;
    c3,t,2
).

start(s).
end(t).

before(b3,t).
after(c2,a2).
