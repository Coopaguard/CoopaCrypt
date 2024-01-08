using System;
using System.IO;
using System.Text;
using System.Security.Cryptography;

namespace CoopaCrypt
{
    public static class Extensions
    {
        public static byte[] ToBytes<T>(this T source)
        {
            return System.Text.Json.JsonSerializer.SerializeToUtf8Bytes<T>(source, new System.Text.Json.JsonSerializerOptions()
            {
                ReferenceHandler = System.Text.Json.Serialization.ReferenceHandler.Preserve,
                NumberHandling = System.Text.Json.Serialization.JsonNumberHandling.AllowNamedFloatingPointLiterals,
                IgnoreReadOnlyFields = true,
                IgnoreReadOnlyProperties = true,
            });
        }

        public static T FromBytes<T>(this byte[] source)
        {
            return System.Text.Json.JsonSerializer.Deserialize<T>(new MemoryStream(source));
        }

        public static byte[] HashString(this string src)
        {
            return SHA256.Create().ComputeHash(Encoding.UTF8.GetBytes(src));
        }
    }
}
