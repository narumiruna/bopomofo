#include <bopomofo.h>
// Model a v1 host's mandatory version check. Never call its old constructor.
int main() { return bpm_abi_version() == 1 ? 0 : 1; }
