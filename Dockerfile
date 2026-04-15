FROM registry.access.redhat.com/ubi9/ubi as builder

RUN dnf install -y rust-toolset openssl-devel

WORKDIR /java-provider
COPY Cargo.lock  /java-provider/
COPY Cargo.toml /java-provider/
COPY build.rs  /java-provider/
COPY src  /java-provider/src

RUN --mount=type=cache,id=cagohome,uid=1001,gid=0,mode=0777,target=/root/.cargo cargo build --release

FROM registry.access.redhat.com/ubi9/ubi-minimal

# Install Java Development Kit, Maven, and Gradle for analyzing Java projects
RUN microdnf install -y java-17-openjdk-devel maven tar gzip findutils unzip && \
    microdnf clean all && \
    rm -rf /var/cache/dnf

# Install Gradle (not available in UBI repos)
RUN curl -L https://services.gradle.org/distributions/gradle-8.5-bin.zip -o /tmp/gradle.zip && \
    unzip /tmp/gradle.zip -d /opt && \
    ln -s /opt/gradle-8.5/bin/gradle /usr/local/bin/gradle && \
    rm /tmp/gradle.zip

# Set Java home and update path
ENV JAVA_HOME=/usr/lib/jvm/java-17-openjdk
ENV PATH="${JAVA_HOME}/bin:${PATH}"

# Set up permissions for OpenShift
RUN chgrp -R 0 /home && chmod -R g=u /home

USER 1001

ENV HOME=/home
ENV RUST_LOG=INFO,java_analyzer_provider=DEBUG

WORKDIR /analyzer-lsp
RUN chgrp -R 0 /analyzer-lsp && chmod -R g=u /analyzer-lsp

COPY --from=builder /java-provider/target/release/java-analyzer-provider /usr/local/bin/java-provider

ENTRYPOINT ["/usr/local/bin/java-provider"]
CMD ["9000"]
