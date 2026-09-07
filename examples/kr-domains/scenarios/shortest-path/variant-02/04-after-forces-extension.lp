% Scenario 04: An after/2 constraint forces an extra edge.
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
