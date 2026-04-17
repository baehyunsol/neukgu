Help me with birthday-problem.

I have to use hash functions in a lot of applications, and I'm worried about hash collisions. I want you to write a function that calculates the probability of hash collision.

The function looks like this:

```rs
fn collision_prob(
    // It's N bit hash function.
    hash_bits: u16,

    // There are N elements.
    elements: u64,
) -> f32;  // The probability of collision (in percent).
```

For example, let's say I have 100_000 elements, and add 64-bit hash to each element. Then the probability that there's at least 1 hash collision is `collision_prob(64, 100_000)`.

Assume that the has function is perfectly well-distributed.
