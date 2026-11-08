% Strategic companies: a set of companies is strategic when its members produce
% every product, it is closed under control, and no proper subset has both
% properties. Each answer set is one strategic set.
%
% Every product has four producers, and one company in three is jointly
% controlled by four companies other than itself; repeated companies are
% allowed, as in the standard encoding. A fixed hash of `seed`, the kind, the
% index and the slot chooses every company, so the constants alone determine the
% instance. The hash squares modulo the prime 46337, so every intermediate value
% stays below 2^31.

#const companies = 12.
#const products = 24.
#const seed = 1.

company(1..companies).
product(1..products).

% Kind 0 draws a product's producers in slots 1..4; kind 1 draws whether a
% company is controlled (slot 0) and its controllers (slots 1..4).
slot(0, P, S) :- product(P), S = 1..4.
slot(1, C, S) :- company(C), S = 0..4.
draw(K, A, S, H) :-
    slot(K, A, S),
    H0 = (seed * 7919 + A * 104729 + S * 1299709 + K * 15485863) \ 46337,
    H1 = (H0 * H0 + 12345) \ 46337,
    H2 = (H1 * H1 + 54321) \ 46337,
    H = (H2 * H2 + 7) \ 46337.

producer(P, S, H \ companies + 1) :- draw(0, P, S, H).
% A controller is any company other than the controlled one.
controller(C, S, (C - 1 + H \ (companies - 1) + 1) \ companies + 1) :-
    draw(1, C, S, H), S > 0.
controlled(C) :- draw(1, C, 0, H), H \ 3 = 0.

produced_by(P, C1, C2, C3, C4) :-
    product(P), producer(P, 1, C1), producer(P, 2, C2),
    producer(P, 3, C3), producer(P, 4, C4).
controlled_by(C, C1, C2, C3, C4) :-
    controlled(C), controller(C, 1, C1), controller(C, 2, C2),
    controller(C, 3, C3), controller(C, 4, C4).

% The standard encoding: some producer of every product is strategic, and a
% company whose controllers are all strategic is strategic too.
strat(C1) | strat(C2) | strat(C3) | strat(C4) :- produced_by(P, C1, C2, C3, C4).
strat(C) :-
    controlled_by(C, C1, C2, C3, C4), strat(C1), strat(C2), strat(C3), strat(C4).

#show strat/1.
