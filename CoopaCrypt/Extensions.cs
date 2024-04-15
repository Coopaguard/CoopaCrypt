using System.IO;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace CoopaCrypt
{
    public static class Extensions
    {
        public static readonly JsonSerializerOptions JsonOptions = new ()
            {
                ReferenceHandler = System.Text.Json.Serialization.ReferenceHandler.Preserve,
                NumberHandling = System.Text.Json.Serialization.JsonNumberHandling.AllowNamedFloatingPointLiterals,
                IgnoreReadOnlyFields = true,
                IgnoreReadOnlyProperties = true,
            }; 

        public static byte[] ToBytes<T>(this T source)
        {
            return System.Text.Json.JsonSerializer.SerializeToUtf8Bytes<T>(source, JsonOptions);
        }

        public static T? FromBytes<T>(this byte[] source)
        {
            return System.Text.Json.JsonSerializer.Deserialize<T>(new MemoryStream(source));
        }

        public static byte[] HashString(this string src)
        {
            return SHA256.HashData(Encoding.UTF8.GetBytes(src));
        }
    }
}
