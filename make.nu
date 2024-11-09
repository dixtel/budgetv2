#!/usr/bin/env nu

def main [] {
  print "./make.nu up (start db, adminer)"
  print "./make.nu run (run app)"
}

def "main up" [] {
  docker-compose up -d
}

def "main watch" [] {
  cargo watch -x 'run'
}
