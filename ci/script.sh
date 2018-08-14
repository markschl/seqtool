# This script takes care of testing your crate

set -ex

main() {
    cross build --features=exprtk --target $TARGET

    if [ ! -z $DISABLE_TESTS ]; then
        return
    fi

    RUST_BACKTRACE=1 cross test --features=exprtk --target $TARGET
    cross run --features=exprtk --target $TARGET
}

# we don't run the "test phase" when doing deploys
if [ -z $TRAVIS_TAG ]; then
    main
fi
