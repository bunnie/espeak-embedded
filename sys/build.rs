extern crate cc;

use std::env::set_var;

fn main() {
    set_var("CC", "riscv64-unknown-elf-gcc");  // set the compiler to what's installed on the system

    let espeak_srcs = vec![
        "espeak-ng/src/libespeak-ng/translate.c",
        "espeak-ng/src/libespeak-ng/speech.c",
        "espeak-ng/src/libespeak-ng/wavegen.c",
        "espeak-ng/src/libespeak-ng/synthesize.c",
        "espeak-ng/src/libespeak-ng/dictionary.c",
        "espeak-ng/src/libespeak-ng/voices.c",
        "espeak-ng/src/libespeak-ng/synthdata.c",
        "espeak-ng/src/libespeak-ng/readclause.c",
        "espeak-ng/src/libespeak-ng/numbers.c",
        "espeak-ng/src/libespeak-ng/setlengths.c",
        "espeak-ng/src/libespeak-ng/tr_languages.c",
        "espeak-ng/src/libespeak-ng/encoding.c",
        "espeak-ng/src/libespeak-ng/intonation.c",
        "espeak-ng/src/libespeak-ng/synth_mbrola.c",
        "espeak-ng/src/libespeak-ng/phoneme.c",
        "espeak-ng/src/libespeak-ng/phonemelist.c",
        //"espeak-ng/src/libespeak-ng/compiledict.c",
        "espeak-ng/src/libespeak-ng/mnemonics.c",
        "espeak-ng/src/libespeak-ng/error.c",
        "espeak-ng/src/ucd-tools/src/case.c",
        "espeak-ng/src/ucd-tools/src/categories.c",
        "espeak-ng/src/ucd-tools/src/ctype.c",
        "espeak-ng/src/ucd-tools/src/proplist.c",

        //"espeak-ng/src/ucd-tools/src/scripts.c",
        //"espeak-ng/src/ucd-tools/src/tostring.c",
        //"espeak-ng/src/libespeak-ng/compiledata.c",
        //"espeak-ng/src/libespeak-ng/compilembrola.c",
        //"espeak-ng/src/libespeak-ng/espeak_api.c",
        //"espeak-ng/src/libespeak-ng/ieee80.c",
        //"espeak-ng/src/libespeak-ng/soundicon.c",
        //"espeak-ng/src/libespeak-ng/spect.c",
        //"espeak-ng/src/libespeak-ng/ssml.c",
        "espeak-ng/src/ffi.c",
        "espeak-ng/src/libc.c",
        "espeak-ng/src/scanf.c",
    ];
    let espeak_includes = vec![
        "espeak-ng",
        "espeak-ng/src",
        "espeak-ng/src/include/compat",
        "espeak-ng/src/include/espeak",
        "espeak-ng/src/include/espeak-ng",
        "espeak-ng/src/ucd-tools/src/include",
        "espeak-ng/src/include",
    ];

	let mut base_config = cc::Build::new();
    base_config.target("riscv32imac-unknown-none-elf");

    for inc in espeak_includes {
        base_config.include(inc);
    }

    base_config.opt_level(3);

    for src in espeak_srcs {
        base_config.file(src);
    }
    base_config.define("EMBEDDED", None);
    base_config.define("NO_STD", None);
    // base_config.define("FFI_DEBUG", None);
	base_config.compile("libespeak.a");
}