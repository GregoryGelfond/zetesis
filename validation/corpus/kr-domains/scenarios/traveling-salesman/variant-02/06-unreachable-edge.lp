% Scenario 06: A profitable edge no salesman reaches cannot be attributed to one.
% @expect sat
% @cost { 6 }
% @count optimal 2
%
% One salesman tours s -> a -> s and the other s -> b -> c -> d -> s, for a total
% cost of 6. The arc d -> b carries a cost of -5, so the optimizer actively wants
% it, and neither salesman can legitimately take it. The one holding b, c and d
% already leaves d by d -> s and enters b by s -> b, so d -> b would exceed its
% degree bounds at either end, and dropping d -> s would leave it unable to
% return to the start. The one touring a never visits d or b at all.
%
% Only the no-dangling-edge constraint stops that second salesman from being
% recorded on the arc regardless. Without it the optimum is 1, on an edge set
% where a salesman traverses an arc between two vertices it never visits.
%
% Costs are unconstrained in sign, and this is where that bites: it is the
% structure, not the sign of the costs, that makes each salesman's edge set its
% tour. The two optima are the salesman labellings of the same pair of tours.

#include "../../../encodings/traveling-salesman/variant-02.lp".

vertex(s; a; b; c; d).

salesman(m1; m2).

start(s).

% One tour
edge(s,a,1; a,s,1).

% The other
edge(s,b,1; b,c,1; c,d,1; d,s,1).

% Profitable, and legitimately reachable by neither.
edge(d,b,-5).
