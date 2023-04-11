#!/bin/bash

coins = sui client gas | awk -F ' ' '{print $1}'