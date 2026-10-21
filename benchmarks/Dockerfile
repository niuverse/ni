FROM ghcr.io/astral-sh/uv@sha256:64165e61dd5fed90daa14ba2d17cdb5a49964837ff85431a3b8199d7a9aa98c0
# Prepared by build.py: exact source/dependency locks plus recorded toolchain binaries.
COPY node /usr/local/bin/node
COPY rust /opt/rust
COPY compat /opt/compat
COPY node_modules /opt/agents/node_modules
COPY --chmod=755 python-deps /opt/python
COPY toolchains.json /opt/toolchains.json
COPY package.json package-lock.json /opt/agents/
ENV PATH="/opt/rust/bin:/opt/agents/node_modules/.bin:/opt/python/bin:$PATH" PYTHONPATH=/opt/python LD_LIBRARY_PATH=/opt/compat
COPY niucode /usr/local/bin/niucode
COPY adapter.py /opt/adapter.py
RUN mkdir -p /workspace /home/bench && chmod 1777 /workspace /home/bench
ENV HOME=/home/bench PYTHONDONTWRITEBYTECODE=1 CARGO_NET_OFFLINE=true
WORKDIR /workspace
ENTRYPOINT ["python3","/opt/adapter.py"]

