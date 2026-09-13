"""Score warehouse allocation artifacts against the exhaustive optimum."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from artifacts import main

if __name__ == "__main__":
    sys.exit(main(Path(__file__).resolve().parent))
