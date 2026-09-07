% Scenario 04: An after/2 constraint forces an extra edge.
% @expect sat
% @cost { 3 3 }
% @count optimal 1
% @optimal { start(s), included(s,a,1), included(a,b,1), included(b,t,1), end(t) }
%
% Edges: s -> a (1), a -> t (1), a -> b (1), b -> t (1). Unconstrained, the
% optimum is s -> a -> t at cost 2. after(b,a) requires b to be visited after a,
% which rules that route out; the cheapest path visiting b after a is
% s -> a -> b -> t at cost 3. The constraint costs exactly one extra edge.

#include "../../../encodings/shortest-path/variant-02.lp".

vertex(s; a; b; t).

edge(
    s,a,1;
    a,t,1;
    a,b,1;
    b,t,1
).

start(s).
end(t).

after(b,a).
