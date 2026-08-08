# syntax=docker/dockerfile:1.7

# Pass a locally available immutable reference, for example
# rocker/r-ver@sha256:<digest>. The analysis runner also rejects mutable tags.
ARG R_IMAGE
FROM ${R_IMAGE}

ENV RENV_PATHS_LIBRARY=/opt/renv/library \
    R_LIBS=/opt/renv/library \
    RENV_CONFIG_AUTOLOADER_ENABLED=FALSE \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    TZ=UTC \
    OMP_NUM_THREADS=1 \
    OPENBLAS_NUM_THREADS=1 \
    MKL_NUM_THREADS=1

WORKDIR /benchmark

RUN apt-get update && \
    apt-get install -y --no-install-recommends cmake && \
    rm -rf /var/lib/apt/lists/*

RUN mkdir -p /opt/renv/library

COPY analysis/renv.lock /benchmark/analysis/renv.lock

RUN Rscript -e 'install.packages("https://cloud.r-project.org/src/contrib/Archive/renv/renv_1.1.5.tar.gz", repos = NULL, type = "source")'
RUN Rscript -e 'renv::restore(project = "/benchmark/analysis", lockfile = "/benchmark/analysis/renv.lock", prompt = FALSE)'
RUN Rscript -e 'lock <- jsonlite::fromJSON("/benchmark/analysis/renv.lock", simplifyVector = FALSE); expected <- vapply(lock$Packages, function(record) record$Version, character(1)); matches <- vapply(names(expected), function(package) packageVersion(package) == package_version(expected[[package]]), logical(1)); stopifnot(all(matches)); stopifnot(getRversion() == package_version(lock$R$Version))'

COPY analysis /benchmark/analysis

USER 65532:65532
ENTRYPOINT ["Rscript"]
CMD ["/benchmark/analysis/test-analysis.R"]
