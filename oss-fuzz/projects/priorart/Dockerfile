# OSS-Fuzz build image for priorart's cargo-fuzz targets.
#
# Layout follows google/oss-fuzz/projects/<name>/. Nothing here is submitted
# upstream: this copy exists so the configuration is reviewable and testable
# from the repository it builds.
#
# The source is pinned. OSS-Fuzz normally clones the default branch, which
# would make every crash report depend on whatever HEAD was that day. Bump
# PA_COMMIT and the comment together, in a commit that says why. After the P1
# tag exists, bump it to the tag and add `--locked` to build.sh; until then the
# pinned 0.2.0 commit's fuzz/Cargo.lock names the root package 0.1.2 and the
# build refreshes that one entry.
FROM gcr.io/oss-fuzz-base/base-builder-rust

# 0.2.0, 2026-11-22: the release whose parser suite the fuzz seeds cover.
ARG PA_COMMIT=bcac935f7abb00bd5190320f6cc3018f77c15eca

RUN git clone https://github.com/KhomDev/priorart.git "$SRC/priorart" \
    && cd "$SRC/priorart" \
    && git checkout --detach "$PA_COMMIT"

WORKDIR $SRC/priorart

COPY build.sh $SRC/
