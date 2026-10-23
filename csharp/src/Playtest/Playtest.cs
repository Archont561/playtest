namespace Playtest;

/// <summary>
/// Playtest testing framework facade for .NET.
/// </summary>
public static class Playtest
{
    /// <summary>
    /// Current Playtest framework semantic version.
    /// </summary>
    public const string Version = "0.1.0";

    /// <summary>
    /// Returns the semantic version of the framework.
    /// </summary>
    public static string GetVersion() => Version;
}
