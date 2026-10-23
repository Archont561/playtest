using Xunit;
using Playtest;

namespace Playtest.Tests;

public class VersionTests
{
    [Fact]
    public void TestVersionAccessible()
    {
        Assert.Equal("0.1.0", Playtest.Version);
        Assert.Equal("0.1.0", Playtest.GetVersion());
    }
}
