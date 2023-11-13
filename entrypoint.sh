#!/bin/sh

test_bin=`find target/release/deps/ -maxdepth 1 -perm -111 -type f | grep cartesi_machine_client_tests`
($test_bin)
