#include <bopomofo.h>
static_assert(BPM_ABI_VERSION == 1, "Unsupported C header version");
int main() { return bpm_abi_version() == BPM_ABI_VERSION ? 0 : 1; }
