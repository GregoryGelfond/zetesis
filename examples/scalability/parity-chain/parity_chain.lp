% Two chains compute the exclusive-or of the same inputs x(1..p), in two
% orders, through three-variable gates with auxiliary atoms, and the program
% requires the two results' exclusive-or to be `parity`. With `parity = 1` the
% results must differ, which they cannot: the program is unsatisfiable. With
% `parity = 0` every assignment of the inputs extends to exactly one answer
% set, 2^p in all.
%
% Each gate and the final comparison state their parity as one integrity
% constraint per forbidden pattern, the formula's clauses. Resolution, and so a
% solver that learns clauses, needs exponentially many steps to refute two such
% chains in independent orders; elimination over GF(2) refutes them in
% polynomial time.
%
% The second chain reads the inputs in the order of a permutation pi of
% 0..p - 1: its K-th input is x(pi(K - 1) + 1), with pi(I) = (I + seed)^3 + 7
% mod p. It is a bijection when p is a prime whose remainder modulo 3 is not 1
% (2, 3, 5, 11, 17, 23, 29, ...), because cubing permutes the residues modulo
% such a prime. Every intermediate value stays below p^2.

#const p = 5.
#const seed = 1.
#const parity = 1.

% The atoms x(1..p) are the inputs. The first chain's running results are
% x(p + 1..2p - 1), the second's x(2p..3p - 2).
variable(1..3 * p - 2).
{ x(V) } :- variable(V).

% input(H, K, I): chain H's K-th input is x(I).
input(1, K, K) :- K = 1..p.
input(2, K, M + 1) :-
    K = 1..p,
    U = (K - 1 + seed) \ p,
    S = (U * U) \ p,
    M = (S * U + 7) \ p.

% result(H, K, R): chain H's result after its first K inputs is x(R). The
% first input is its own result.
result(H, 1, I) :- input(H, 1, I).
result(1, K, p + K - 1) :- K = 2..p.
result(2, K, 2 * p + K - 2) :- K = 2..p.

% gate(A, B, C): x(C) is the exclusive-or of x(A) and x(B), that is
% x(A) xor x(B) xor x(C) = 0.
gate(A, B, C) :- result(H, K - 1, A), input(H, K, B), result(H, K, C), K > 1.

% The two final results, and the parity their exclusive-or must have.
final(A, B) :- result(1, p, A), result(2, p, B).
target(parity).

% A gate forbids an odd number of its three atoms true.
:- gate(A, B, C), x(A), not x(B), not x(C).
:- gate(A, B, C), not x(A), x(B), not x(C).
:- gate(A, B, C), not x(A), not x(B), x(C).
:- gate(A, B, C), x(A), x(B), x(C).

% The final results' exclusive-or is the target.
:- final(A, B), target(1), x(A), x(B).
:- final(A, B), target(1), not x(A), not x(B).
:- final(A, B), target(0), x(A), not x(B).
:- final(A, B), target(0), not x(A), x(B).

% An answer set shows the inputs it holds.
on(I) :- x(I), I <= p.

#show on/1.
