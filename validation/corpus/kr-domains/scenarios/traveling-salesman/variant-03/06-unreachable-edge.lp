% Scenario 06: A profitable edge no salesman reaches cannot be attributed to one.
% @expect sat
% @cost { 6 }
% @count optimal 1
%
% m1 tours p -> a -> p from its depot and m2 tours q -> b -> c -> d -> q from its
% own, for a total cost of 6. The arc d -> b carries a cost of -5, so the
% optimizer actively wants it, and neither salesman can legitimately take it. m2
% already leaves d by d -> q and enters b by q -> b, so d -> b would exceed its
% degree bounds at either end, and dropping d -> q would leave it unable to
% return to its depot. m1 departs p and can reach nothing but a.
%
% Only the no-dangling-edge constraint stops m1 from being recorded on the arc
% regardless. Without it the optimum is 1, on an edge set where m1 traverses an
% arc between two vertices it never visits.
%
% Costs are unconstrained in sign, and this is where that bites: it is the
% structure, not the sign of the costs, that makes each salesman's edge set its
% tour. The optimum is unique because the depots are distinct, so unlike
% variant-02 there is no salesman-labelling symmetry.

#include "../../../encodings/traveling-salesman/variant-03.lp".

vertex(p; q; a; b; c; d).

salesman(m1; m2).
depot(m1,p). depot(m2,q).

% m1's tour
edge(p,a,1; a,p,1).

% m2's tour
edge(q,b,1; b,c,1; c,d,1; d,q,1).

% Profitable, and legitimately reachable by neither.
edge(d,b,-5).
