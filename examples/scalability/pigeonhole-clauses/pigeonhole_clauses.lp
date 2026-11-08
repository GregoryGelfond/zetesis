% The pigeonhole principle as clauses. There are `holes` holes and
% holes + `extra` pigeons; every pigeon occupies some hole, and no hole holds
% two pigeons. With `extra = 1` that is impossible: the program is
% unsatisfiable. With `extra = 0` every pigeon occupies exactly one hole, and
% the answer sets are the holes! one-to-one placements.
%
% Each condition is written as integrity constraints over the choice atoms, the
% formula's clauses: one per pigeon that it occupies no hole, and one per pair
% of pigeons sharing a hole. Resolution, and so a solver that learns clauses,
% needs exponentially many steps to refute them. Counting the incidences once
% by pigeon and once by hole refutes them at once, since the pigeons need more
% places than the holes offer. The scalability corpus's pigeonhole.lp states
% the same principle with a choice bound.

#const holes = 4.
#const extra = 1.

hole(1..holes).
pigeon(1..holes + extra).

% A pigeon may occupy any of the holes.
{ in(P, H) } :- pigeon(P), hole(H).

% Every pigeon occupies some hole.
:- pigeon(P), not in(P, H) : hole(H).

% No two pigeons share a hole.
:- hole(H), in(P, H), in(Q, H), P < Q.

#show in/2.
