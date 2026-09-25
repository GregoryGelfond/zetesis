% Pigeonhole, parametric and unsatisfiable by construction: h + 1 pigeons placed
% into h holes cannot avoid sharing one. It is a clean scaling test for *proving
% unsatisfiability* rather than enumerating models -- refuting it takes work that
% grows steeply with h. One scaling knob, #const h (holes); pigeons are h + 1.

#const h = 7.

hole(1..h).
pigeon(1..h + 1).

% Each pigeon occupies exactly one hole.
1 { in(P,H) : hole(H) } 1 :- pigeon(P).

% No two pigeons share a hole.
:- in(P1,H), in(P2,H), P1 < P2.

#show in/2.
