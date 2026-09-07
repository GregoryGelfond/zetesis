% Scenario 04: A window on a depot bounds the tour close and rules it out (UNSAT).
% @expect unsat
%
% Single salesman, depot a, customer b, on a 2-cycle a -> b -> a with edge time 3
% each way: the tour takes 6. time_window(a,0,5) bounds the tour close to 5, so the
% only structurally valid tour is rejected. This exercises the depot-window
% semantics: the window applies to a's tour-close arrival (visit_time(a,s1,6) in
% clingo; visit_time(a,s1) linked to tour_time(s1) in clingcon), so it propagates
% to rule out any close time over 5.

#include "../../../encodings/traveling-salesman/variant-04.lp".

vertex(a; b).

edge(
    a,b,1;
    b,a,1
).

edge_time(
    a,b,3;
    b,a,3
).

salesman(s1).
depot(s1,a).

time_window(a,0,5).
