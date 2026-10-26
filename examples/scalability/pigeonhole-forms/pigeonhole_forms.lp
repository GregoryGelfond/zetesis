% The pigeonhole principle in two encodings. There are `holes` holes and
% holes + `extra` pigeons; every pigeon occupies some hole, and no hole holds
% two pigeons. With `extra = 1` that is impossible: the program is
% unsatisfiable. With `extra = 0` every pigeon occupies exactly one hole, and
% the answer sets are the holes! one-to-one placements.
%
% `form` selects the encoding of the same two conditions:
% - form 1, clauses: one integrity constraint per pigeon that it occupies no
%   hole, and one per pair of pigeons sharing a hole;
% - form 2, counts: one count bound per pigeon and per hole.
% Resolution, and so a solver that learns clauses, needs exponentially many
% steps to refute either; counting both sides of the incidence refutes them at
% once, since the pigeons need more places than the holes offer.

#const holes = 4.
#const extra = 1.
#const form = 1.

hole(1..holes).
pigeon(1..holes + extra).
encoding(form).

% A pigeon may occupy any of the holes.
{ in(P, H) } :- pigeon(P), hole(H).

% Form 1: every pigeon occupies some hole, and no two pigeons share one.
:- encoding(1), pigeon(P), not in(P, H) : hole(H).
:- encoding(1), hole(H), in(P, H), in(Q, H), P < Q.

% Form 2: every pigeon occupies at least one hole, and every hole holds at most
% one pigeon.
:- encoding(2), pigeon(P), #count { H : in(P, H) } < 1.
:- encoding(2), hole(H), #count { P : in(P, H) } > 1.

#show in/2.
