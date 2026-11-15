import argparse
from compute import count_primes


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("limit", type=int)
    args = parser.parse_args()
    print(count_primes(args.limit))
