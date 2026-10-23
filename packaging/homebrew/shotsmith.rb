cask "shotsmith" do
  version "0.1.0"
  # Paste the dmg line from the release SHA256SUMS before submitting this cask.
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/Smithey/shotsmith/releases/download/v#{version}/Shotsmith_#{version}_aarch64.dmg"
  name "Shotsmith"
  desc "Screenshot and annotation tool"
  homepage "https://shotsmith.app"

  app "Shotsmith.app"

  zap trash: [
    "~/Library/Application Support/Shotsmith",
  ]
end
