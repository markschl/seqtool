# This script takes care of testing your crate

set -ex

main() {
    cross build --features=exprtk --target $TARGET
    cross build --features=exprtk --target $TARGET --release

    if [ ! -z $DISABLE_TESTS ]; then
        return
    fi

    cross test --features=exprtk --target $TARGET
    cross test --features=exprtk --target $TARGET --release

    cross run --features=exprtk --target $TARGET
    cross run --features=exprtk --target $TARGET --release
}

# we don't run the "test phase" when doing deploys
if [ -z $TRAVIS_TAG ]; then
    main
fi
