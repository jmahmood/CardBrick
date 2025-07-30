#!/usr/bin/env python3
"""
Setup script for CardBrick CLI
"""

from setuptools import setup, find_packages

setup(
    name="cardbrick-cli",
    version="0.1.0",
    description="Command-line tools for CardBrick flashcard management",
    author="CardBrick Team",
    packages=find_packages(),
    install_requires=[
        "click>=8.0.0",
        "pandas>=2.0.0", 
        "markdown>=3.4.0",
        "pydantic>=2.0.0",
    ],
    entry_points={
        "console_scripts": [
            "cardbrick-cli=cardbrick_cli.cli:main",
        ],
    },
    python_requires=">=3.8",
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: End Users/Desktop",
        "License :: OSI Approved :: GPL License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
    ],
)