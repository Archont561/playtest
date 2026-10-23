using Xunit;
using Playtest;

namespace Playtest.Tests;

public class VersionTests
{
    [Fact]
    public void TestVersionAccessible()
    {
        Assert.Matches(@"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$", Playtest.Version);
        Assert.Equal(Playtest.Version, Playtest.GetVersion());
    }
}
