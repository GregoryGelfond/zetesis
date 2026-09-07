% Scenario 05: Three depots, mixed time windows and revisit intervals.
% @expect sat
% @cost { 6 }
% @note  clingcon leaves the CSP visit-times free, so the optimal class is intractable to enumerate; only the optimal cost is asserted here
%
% Three disjoint 2-cycles with asymmetric edge times, a time window on one cluster
% and revisit intervals on the other two, exercising the v04 window machinery and
% the v05 surveillance constraint together:
%   s1: a, p. a -> p time 2, p -> a time 1. tour_time 3; max_revisit_interval(p,5) holds (3 <= 5).
%   s2: b, q. b -> q time 1, q -> b time 3. tour_time 4; time_window(q,0,2) holds (visit at 1).
%   s3: c, r. c -> r time 3, r -> c time 2. tour_time 5; max_revisit_interval(r,5) holds at the bound (5 <= 5).
% No cross-cluster edges, so each salesman patrols its own pair. Total cost 6 (six
% unit edges).

#include "../../../encodings/traveling-salesman/variant-05.lp".

vertex(a; b; c; p; q; r).

edge(
    a,p,1;
    p,a,1;
    b,q,1;
    q,b,1;
    c,r,1;
    r,c,1
).

edge_time(
    a,p,2;
    p,a,1;
    b,q,1;
    q,b,3;
    c,r,3;
    r,c,2
).

salesman(s1; s2; s3).
depot(
    s1,a;
    s2,b;
    s3,c
).

max_revisit_interval(p,5).
time_window(q,0,2).
max_revisit_interval(r,5).
